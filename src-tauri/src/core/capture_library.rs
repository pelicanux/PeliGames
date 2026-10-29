//! Video filenames, cached thumbnails and metadata. Renaming preserves video contents.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::{fs::OpenOptionsExt, process::CommandExt};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};
static GENERATION: Mutex<()> = Mutex::new(());
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Details {
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub codec: Option<String>,
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub pending: bool,
    pub warning: Option<String>,
}
fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn cache() -> Result<PathBuf, String> {
    let root = super::super::paths::app_path("CaptureLibrary")?;
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}
fn cache_base(root:&Path,file:&Path,identity:&str)->Result<PathBuf,String>{
    let game=file.parent().and_then(|p|p.file_name()).unwrap_or_default().to_string_lossy();
    let game=game.rsplit_once('-').filter(|(_,suffix)|suffix.len()==12&&suffix.bytes().all(|b|b.is_ascii_hexdigit())).map(|(name,_)|name).unwrap_or(&game);
    let base=super::super::paths::names::allocate(root,&format!("{game}-{}",title(file)),&identity)?;
    let legacy=root.join(format!("{identity}.json"));
    if !base.with_extension("json").exists()&&legacy.is_file(){
        let bytes=fs::read(&legacy).map_err(|e|e.to_string())?;
        if let Ok(mut details)=serde_json::from_slice::<Details>(&bytes){
            let old=root.join(format!("{identity}.jpg"));let image=base.with_extension("jpg");
            if old.is_file()&&!image.exists(){fs::rename(old,&image).map_err(|e|e.to_string())?;details.thumbnail=Some(image.to_string_lossy().into_owned());}
            write_json(&base.with_extension("json"),&details)?;fs::remove_file(legacy).map_err(|e|e.to_string())?;
        }
    }
    Ok(base)
}
pub fn validate_file(folder: &Path, path: &Path) -> Result<PathBuf, String> {
    let folder = folder.canonicalize().map_err(|e| e.to_string())?;
    if fs::symlink_metadata(path)
        .map_err(|e| e.to_string())?
        .file_type()
        .is_symlink()
    {
        return Err("Selecione uma gravação da pasta deste jogo.".into());
    }
    let file = path.canonicalize().map_err(|e| e.to_string())?;
    let extension = file
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if file.parent() != Some(folder.as_path())
        || !file.is_file()
        || !["mp4", "mkv", "webm"].contains(&extension.as_str())
    {
        return Err("Selecione uma gravação da pasta deste jogo.".into());
    }
    Ok(file)
}
pub fn title(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temporary)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec(value).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temporary, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[derive(Debug, Serialize)]
pub struct Renamed {
    pub title: String,
    pub name: String,
    pub path: String,
}
fn rename_file(folder: &Path, path: &Path, title: &str) -> Result<Renamed, String> {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let file = validate_file(folder, path)?;
    let title = title.trim();
    if title.is_empty()
        || title.chars().count() > 120
        || title
            .chars()
            .any(|c| c.is_control() || "/\\:*?\"<>|".contains(c))
        || title == "."
        || title == ".."
        || title.ends_with('.')
    {
        return Err(
            "Use um nome de 1 a 120 caracteres, sem barras ou caracteres inválidos para arquivos."
                .into(),
        );
    }
    let extension = file
        .extension()
        .and_then(|s| s.to_str())
        .ok_or("Extensão do vídeo indisponível.")?;
    let name = format!("{title}.{extension}");
    if name.len() > 255 {
        return Err("O nome do arquivo ficou muito longo. Use um título menor.".into());
    }
    let destination = file.with_file_name(&name);
    if destination != file {
        let old = CString::new(file.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let new = CString::new(destination.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        // Atomic and no-replace: an existing recording is never overwritten, even on a race.
        if unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                old.as_ptr(),
                libc::AT_FDCWD,
                new.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            let error = std::io::Error::last_os_error();
            return Err(if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Já existe um vídeo com esse nome nesta pasta. Escolha outro título.".into()
            } else {
                format!("Não foi possível renomear o vídeo: {error}")
            });
        }
    }
    Ok(Renamed {
        title: title.into(),
        name,
        path: destination.to_string_lossy().into_owned(),
    })
}
pub fn rename(folder: &Path, path: &Path, title: &str) -> Result<Renamed, String> {
    let file = validate_file(folder, path)?;
    let old_key = fingerprint(&file).ok();
    let renamed = rename_file(folder, &file, title)?;
    // Retain the cached thumbnail after a rename; cache failure must not undo a successful rename.
    if let (Some(old_key), Ok(new_key), Ok(root)) =
        (old_key, fingerprint(Path::new(&renamed.path)), cache())
    {
        if old_key != new_key {
            if let Some(old_base) = super::super::paths::names::find(&root,&old_key) {
            if let Ok(bytes) = fs::read(old_base.with_extension("json")) {
                if let Ok(details) = serde_json::from_slice::<Details>(&bytes) {
                    if let Ok(new_base)=cache_base(&root,Path::new(&renamed.path),&new_key){let _=write_json(&new_base.with_extension("json"),&details);}
                }
            }
            }
        }
    }
    Ok(renamed)
}
fn fingerprint(path: &Path) -> Result<String, String> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    Ok(digest(&format!(
        "{}:{}:{:?}",
        path.display(),
        m.len(),
        m.modified().ok()
    )))
}
fn run(program: &str, args: &[String], timeout: Duration) -> Result<Vec<u8>, String> {
    let mut command = Command::new(program);
    super::super::env_utils::host_environment(&mut command);
    unsafe {
        command.pre_exec(|| {
            libc::setpriority(libc::PRIO_PROCESS, 0, 19);
            Ok(())
        });
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            "Instale FFmpeg (ffmpeg e ffprobe) para gerar miniaturas e informações dos vídeos."
                .to_string()
        })?;
    let stdout = child.stdout.take().ok_or("Saída de vídeo indisponível.")?;
    let reader = thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout.take(65537).read_to_end(&mut bytes);
        (result, bytes)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(30)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = reader.join();
                return Err("Não foi possível analisar este vídeo a tempo.".into());
            }
        }
    };
    let (read, bytes) = reader.join().map_err(|_| "Falha ao analisar o vídeo.")?;
    if !status.success() || read.is_err() || bytes.len() > 65536 {
        return Err("Não foi possível gerar as informações deste vídeo.".into());
    }
    Ok(bytes)
}
fn parse_probe(bytes: &[u8]) -> Result<Details, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let stream = value["streams"]
        .as_array()
        .and_then(|v| v.first())
        .ok_or("Vídeo sem faixa de imagem.")?;
    let duration = value["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v >= 0.);
    let fps = stream["avg_frame_rate"]
        .as_str()
        .and_then(|s| s.split_once('/'))
        .and_then(|(n, d)| Some(n.parse::<f64>().ok()? / d.parse::<f64>().ok()?))
        .filter(|v| v.is_finite() && *v > 0.);
    Ok(Details {
        duration,
        width: stream["width"].as_u64().and_then(|v| v.try_into().ok()),
        height: stream["height"].as_u64().and_then(|v| v.try_into().ok()),
        fps,
        codec: stream["codec_name"].as_str().map(str::to_owned),
        ..Default::default()
    })
}
pub fn details(folder: &Path, path: &Path, generate: bool) -> Result<Details, String> {
    details_at(&cache()?,folder,path,generate)
}
fn details_at(root:&Path,folder:&Path,path:&Path,generate:bool)->Result<Details,String>{
    let file=validate_file(folder,path)?;
    let identity=fingerprint(&file)?;
    let base=cache_base(root,&file,&identity)?;
    let key=base.file_name().unwrap().to_string_lossy();
    let json=base.with_extension("json");
    if let Ok(bytes) = fs::read(&json) {
        if let Ok(value) = serde_json::from_slice::<Details>(&bytes) {
            if value
                .thumbnail
                .as_ref()
                .is_some_and(|path| Path::new(path).is_file())
                || !generate
            {
                return Ok(value);
            }
        }
    }
    if !generate {
        return Ok(Details {
            pending: true,
            ..Default::default()
        });
    }
    let _guard = GENERATION.lock().map_err(|e| e.to_string())?;
    if let Ok(bytes) = fs::read(&json) {
        if let Ok(value) = serde_json::from_slice::<Details>(&bytes) {
            if value
                .thumbnail
                .as_ref()
                .is_some_and(|path| Path::new(path).is_file())
                || !generate
            {
                return Ok(value);
            }
        }
    }
    let mut metadata = match run(
        "ffprobe",
        &[
            "-v".into(),
            "error".into(),
            "-protocol_whitelist".into(),
            "file,pipe".into(),
            "-select_streams".into(),
            "v:0".into(),
            "-show_entries".into(),
            "stream=width,height,avg_frame_rate,codec_name:format=duration".into(),
            "-of".into(),
            "json".into(),
            file.to_string_lossy().into_owned(),
        ],
        Duration::from_secs(8),
    )
    .and_then(|bytes| parse_probe(&bytes))
    {
        Ok(v) => v,
        Err(e) => {
            return Ok(Details {
                warning: Some(e),
                ..Default::default()
            })
        }
    };
    let thumbnail = root.join(format!("{key}.jpg"));
    let temporary = root.join(format!("{key}-{}.jpg", uuid::Uuid::new_v4()));
    let seek = (metadata.duration.unwrap_or(0.) * 0.1).min(2.);
    match run(
        "ffmpeg",
        &[
            "-nostdin".into(),
            "-v".into(),
            "error".into(),
            "-protocol_whitelist".into(),
            "file,pipe".into(),
            "-ss".into(),
            seek.to_string(),
            "-threads".into(),
            "1".into(),
            "-i".into(),
            file.to_string_lossy().into_owned(),
            "-frames:v".into(),
            "1".into(),
            "-threads".into(),
            "1".into(),
            "-vf".into(),
            "scale=480:270:force_original_aspect_ratio=decrease".into(),
            "-q:v".into(),
            "5".into(),
            temporary.to_string_lossy().into_owned(),
        ],
        Duration::from_secs(12),
    ) {
        Ok(_) if temporary.is_file() => {
            fs::rename(&temporary, &thumbnail).map_err(|e| e.to_string())?;
            metadata.thumbnail = Some(thumbnail.to_string_lossy().into_owned());
        }
        Ok(_) => metadata.warning = Some("Miniatura indisponível para este vídeo.".into()),
        Err(e) => metadata.warning = Some(e),
    }
    let _ = fs::remove_file(temporary);
    // A recording still being written must not get cached as a completed clip.
    if fingerprint(&file)? == identity {
        write_json(&json, &metadata)?;
    }
    Ok(metadata)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn metadata_uses_real_dimensions_and_handles_unknown_rates() {
        let m=parse_probe(br#"{"streams":[{"width":1920,"height":1080,"avg_frame_rate":"60000/1001","codec_name":"av1"}],"format":{"duration":"60.25"}}"#).unwrap();
        assert_eq!(m.width, Some(1920));
        assert_eq!(m.duration, Some(60.25));
        assert!((m.fps.unwrap() - 59.94).abs() < 0.01);
        assert!(parse_probe(br#"{"streams":[{"avg_frame_rate":"0/0"}]}"#)
            .unwrap()
            .fps
            .is_none());
    }
    #[test]
    fn renames_file_preserves_contents_extension_and_rejects_collisions_and_escaping_paths() {
        let root = std::env::temp_dir().join(format!("peli-library-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        let folder = root.join("game");
        fs::create_dir(&folder).unwrap();
        let video = folder.join("clip.mkv");
        fs::write(&video, b"original").unwrap();
        let saved = rename_file(&folder, &video, "  Minha partida  ").unwrap();
        let renamed = PathBuf::from(saved.path);
        assert_eq!(saved.name, "Minha partida.mkv");
        assert_eq!(saved.title, "Minha partida");
        assert!(!video.exists());
        assert_eq!(fs::read(&renamed).unwrap(), b"original");
        assert_eq!(title(&renamed), "Minha partida");
        assert_eq!(
            rename_file(&folder, &renamed, "Minha partida")
                .unwrap()
                .name,
            saved.name
        );
        let occupied = folder.join("Ocupado.mkv");
        fs::write(&occupied, b"another recording").unwrap();
        assert!(rename_file(&folder, &renamed, "Ocupado")
            .unwrap_err()
            .contains("Já existe"));
        assert_eq!(fs::read(&occupied).unwrap(), b"another recording");
        for invalid in [
            "",
            ".",
            "..",
            "../outro",
            "outro/clip",
            "outro\\clip",
            "novo\ntitulo",
            "nome.",
        ] {
            assert!(rename_file(&folder, &renamed, invalid).is_err());
            assert_eq!(fs::read(&renamed).unwrap(), b"original");
        }
        let outside = root.join("outside.mkv");
        fs::write(&outside, b"other").unwrap();
        assert!(rename_file(&folder, &outside, "Nome").is_err());
        std::os::unix::fs::symlink(&outside, folder.join("link.mkv")).unwrap();
        assert!(rename_file(&folder, &folder.join("link.mkv"), "Nome").is_err());
        assert_eq!(fs::read(&outside).unwrap(), b"other");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn thumbnail_cache_uses_the_clip_title_and_reuses_legacy_metadata() {
        let root=std::env::temp_dir().join(format!("peli-clip-cache-{}",uuid::Uuid::new_v4()));
        let game=root.join("Tomb_Raider-abcdef123456");let cache=root.join("cache");fs::create_dir_all(&game).unwrap();fs::create_dir(&cache).unwrap();
        let video=game.join("Replay-final.mkv");fs::write(&video,b"video").unwrap();let identity=fingerprint(&video).unwrap();
        let image=cache.join(format!("{identity}.jpg"));fs::write(&image,b"thumbnail").unwrap();
        let saved=Details{thumbnail:Some(image.to_string_lossy().into_owned()),width:Some(1920),height:Some(1080),..Default::default()};
        write_json(&cache.join(format!("{identity}.json")),&saved).unwrap();let base=cache_base(&cache,&video,&identity).unwrap();
        assert_eq!(base.file_name().unwrap(),"tomb-raider-replay-final");assert_eq!(cache_base(&cache,&video,&identity).unwrap(),base);
        let value:Details=serde_json::from_slice(&fs::read(base.with_extension("json")).unwrap()).unwrap();
        assert_eq!((value.width,value.height),(Some(1920),Some(1080)));assert_eq!(fs::read(value.thumbnail.unwrap()).unwrap(),b"thumbnail");assert!(!cache.join(format!("{identity}.json")).exists());fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "requires FFmpeg and ffprobe"]
    fn generated_readable_cache_persists_and_is_reused_without_regeneration(){
        let root=std::env::temp_dir().join(format!("peli-video-generation-{}",uuid::Uuid::new_v4()));let game=root.join("Game");let cache=root.join("cache");fs::create_dir_all(&game).unwrap();fs::create_dir(&cache).unwrap();
        let video=game.join("sample.mp4");
        let status=Command::new("ffmpeg").args(["-nostdin","-v","error","-f","lavfi","-i","color=size=32x32:rate=2","-frames:v","4","-c:v","mpeg4"]).arg(&video).status().unwrap();assert!(status.success());
        let details=details_at(&cache,&game,&video,true).unwrap();assert_eq!((details.width,details.height),(Some(32),Some(32)));assert!(details.thumbnail.as_ref().is_some_and(|p|Path::new(p).is_file()),"{:?}",details.warning);assert!(cache.join("game-sample.json").is_file());
        let reused=details_at(&cache,&game,&video,false).unwrap();assert_eq!(details.thumbnail,reused.thumbnail);assert!(!reused.pending);assert_eq!(reused.width,Some(32));fs::remove_dir_all(root).unwrap();
    }

}
