use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Controls {
    pub multiplier: u8,
    pub flow_scale: f32,
    pub performance: bool,
    pub adaptive: bool,
    pub target_fps: u16,
    pub paused: bool,
    pub base_fps_cap: u16,
    pub refresh_threshold: u16,
    pub scaling_enabled: bool,
    pub scaling_method: String,
    pub scaling_sharpness: f32,
    pub ultra_performance: bool,
}
impl Default for Controls {
    fn default() -> Self { Self {multiplier:2,flow_scale:1.,performance:false,adaptive:false,target_fps:60,paused:false,base_fps_cap:0,refresh_threshold:0,scaling_enabled:false,scaling_method:"native".into(),scaling_sharpness:0.8,ultra_performance:false} }
}
impl Controls {
    pub fn validate(&self) -> Result<(), String> {
        if self.base_fps_cap>1000 || self.refresh_threshold>1000
            || !self.scaling_sharpness.is_finite() || !(0.0..=1.).contains(&self.scaling_sharpness)
            || !["native","mako","ls1","ls1-performance"].contains(&self.scaling_method.as_str())
            || !(2..=5).contains(&self.multiplier)
            || !(10..=1000).contains(&self.target_fps)
            || !self.flow_scale.is_finite()
            || !(0.25..=1.).contains(&self.flow_scale)
        {
            return Err("Valores fora dos limites do MAKO.".into());
        }
        Ok(())
    }
}
pub struct Controller {
    pub current: Arc<Mutex<(Controls, String)>>,
    pub sender: mpsc::SyncSender<Controls>,
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("Perfil excede o limite de leitura.".into());
    }
    Ok(bytes)
}
fn atomically(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let parent = path.parent().ok_or("Destino inválido")?;
    let temp = parent.join(format!(
        ".overlay-{}-{}.tmp",
        std::process::id(),
        crate::nonce()
    ));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}
fn initial(path: &Path) -> Result<Controls, String> {
    let value: serde_json::Value =
        serde_json::from_slice(&read(&path.with_extension("json"))?).map_err(|e| e.to_string())?;
    if value["backend"] != "mako" || value["enabled"] != true || (value["overlay_enabled"] != true && std::env::var("PELI_OVERLAY_GENERAL").ok().as_deref()!=Some("1")) {
        return Err("Overlay não habilitado neste perfil MAKO.".into());
    }
    let controls = Controls {
        multiplier: value["multiplier"]
            .as_u64()
            .and_then(|v| u8::try_from(v).ok())
            .ok_or("Multiplicador inválido")?,
        flow_scale: value["flow_scale"].as_f64().ok_or("Escala inválida")? as f32,
        performance: value["performance"].as_bool().unwrap_or(false),
        adaptive: value["adaptive"].as_bool().unwrap_or(false),
        target_fps: value["target_fps"]
            .as_u64()
            .and_then(|v| u16::try_from(v).ok())
            .ok_or("Alvo inválido")?,
        paused: !value["live_enabled"].as_bool().unwrap_or(true),
        ..Controls::default()
    };
    // The watched TOML is authoritative, including edits made by the launcher.
    // live.json is an output/cache, never a second owner of MAKO settings.
    let document: toml::Value=std::str::from_utf8(&read(path)?).map_err(|e|e.to_string())?.parse::<toml::Value>().map_err(|e|e.to_string())?;
    let profile=document.get("profile").and_then(toml::Value::as_array).and_then(|profiles|profiles.iter().find(|p|p.get("name").and_then(toml::Value::as_str)==Some("PeliGames"))).ok_or("Perfil PeliGames ausente")?;
    let mut merged=serde_json::to_value(controls).map_err(|e|e.to_string())?;
    for (source,target) in [("multiplier","multiplier"),("flow_scale","flow_scale"),("performance_mode","performance"),("adaptive","adaptive"),("target_fps","target_fps"),("base_fps_cap","base_fps_cap"),("frame_generation_refresh_threshold","refresh_threshold"),("scaling_enabled","scaling_enabled"),("scaling_method","scaling_method"),("scaling_sharpness","scaling_sharpness"),("ultra_performance","ultra_performance")] {
        if let Some(value)=profile.get(source) {merged[target]=serde_json::to_value(value).map_err(|e|e.to_string())?;}
    }
    if let Some(value)=profile.get("frame_generation_enabled") {merged["paused"]=(!value.as_bool().ok_or("Estado de geração inválido")?).into();}
    let controls:Controls=serde_json::from_value(merged).map_err(|e|e.to_string())?;
    controls.validate()?;
    Ok(controls)
}
fn save(path: &Path, c: &Controls) -> Result<(), String> {
    c.validate()?;
    let current=initial(path)?;
    if !current.scaling_enabled && (c.scaling_method!=current.scaling_method || c.scaling_sharpness!=current.scaling_sharpness) {
        return Err("Ative a escala no launcher e reinicie o jogo antes de ajustá-la no overlay.".into());
    }
    let bytes = read(path)?;
    let mut document = std::str::from_utf8(&bytes)
        .map_err(|e| e.to_string())?
        .parse::<toml::Value>()
        .map_err(|e| e.to_string())?;
    if document.get("version").and_then(toml::Value::as_integer) != Some(2) {
        return Err("Formato MAKO desconhecido.".into());
    }
    let profiles = document
        .get_mut("profile")
        .and_then(toml::Value::as_array_mut)
        .ok_or("Perfil MAKO ausente")?;
    let profile = profiles
        .iter_mut()
        .find(|p| p.get("name").and_then(toml::Value::as_str) == Some("PeliGames"))
        .and_then(toml::Value::as_table_mut)
        .ok_or("Perfil PeliGames ausente")?;
    for (key, value) in [
        ("multiplier", toml::Value::Integer(c.multiplier.into())),
        (
            "adaptive_max_multiplier",
            toml::Value::Integer(c.multiplier.into()),
        ),
        ("target_fps", toml::Value::Integer(c.target_fps.into())),
        ("flow_scale", toml::Value::Float(c.flow_scale as f64)),
        ("performance_mode", toml::Value::Boolean(c.performance)),
        ("adaptive", toml::Value::Boolean(c.adaptive)),
        ("frame_generation_enabled", toml::Value::Boolean(!c.paused)),
        ("base_fps_cap",toml::Value::Integer(c.base_fps_cap.into())),
        ("frame_generation_refresh_threshold",toml::Value::Integer(c.refresh_threshold.into())),
    ] {
        profile.insert(key.into(), value);
    }
    if current.scaling_enabled {
        profile.insert("scaling_method".into(),toml::Value::String(c.scaling_method.clone()));
        profile.insert("scaling_sharpness".into(),toml::Value::Float(c.scaling_sharpness as f64));
    }
    // Re-read startup settings before every update; this is a live edit, not a
    // replacement of startup settings, DLL selection, runtime, or Steam options.
    initial(path)?;
    atomically(
        path,
        toml::to_string(&document)
            .map_err(|e| e.to_string())?
            .as_bytes(),
    )?;
    atomically(
        &path.with_extension("live.json"),
        &serde_json::to_vec(c).map_err(|e| e.to_string())?,
    )
}
impl Controller {
    pub fn new() -> Result<Self, String> {
        use std::os::unix::fs::MetadataExt;
        let raw = std::env::var_os("MAKO_CONFIG").ok_or("MAKO_CONFIG ausente")?;
        let path = PathBuf::from(raw);
        let parent = path.parent().ok_or("Perfil inválido")?;
        if !path.is_absolute()
            || path.extension().and_then(|v| v.to_str()) != Some("toml")
            || parent.file_name().and_then(|v| v.to_str()) != Some("profiles")
            || parent
                .parent()
                .and_then(Path::file_name)
                .and_then(|v| v.to_str())
                != Some("FrameGeneration")
        {
            return Err("Use um perfil de geração do PeliGames.".into());
        }
        for p in [&path, &path.with_extension("json")] {
            let meta = fs::symlink_metadata(p).map_err(|e| e.to_string())?;
            if !meta.is_file()
                || meta.uid() != unsafe { libc::geteuid() }
                || meta.mode() & 0o022 != 0
            {
                return Err("Perfil sem propriedade/permissões seguras.".into());
            }
        }
        let current = Arc::new(Mutex::new((initial(&path)?, String::new())));
        let (sender, receiver) = mpsc::sync_channel::<Controls>(16);
        let shared = current.clone();
        std::thread::Builder::new()
            .name("peli-overlay-controls".into())
            .spawn(move || {
                let mut pending: Option<(Instant, Controls)> = None;
                let mut refresh = Instant::now();
                loop {
                    match receiver.recv_timeout(Duration::from_millis(20)) {
                        Ok(c) => pending = Some((Instant::now(), c)),
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if pending
                        .as_ref()
                        .is_some_and(|(t, _)| t.elapsed() >= Duration::from_millis(200))
                    {
                        if let Some((_, c)) = pending.take() {
                            let result = save(&path, &c);
                            if let Ok(mut state) = shared.lock() {
                                match result {
                                    Ok(()) => *state = (c, String::new()),
                                    Err(e) => state.1 = e,
                                }
                            }
                        }
                    }
                    if pending.is_none() && refresh.elapsed() >= Duration::from_millis(500) {
                        refresh = Instant::now();
                        if let Ok(c) = initial(&path) {
                            if let Ok(mut s) = shared.lock() {
                                s.0 = c;
                            }
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { current, sender })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture {
        directory: PathBuf,
        config: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            use std::os::unix::fs::PermissionsExt;
            let directory =
                std::env::temp_dir().join(format!("peli-overlay-controls-{}", crate::nonce()));
            fs::create_dir(&directory).unwrap();
            let config = directory.join("sample.toml");
            fs::write(&config, b"version=2\n[global]\ndll='/unchanged/Lossless.dll'\nallow_fp16=true\n[[profile]]\nname='Other'\nmultiplier=3\n[[profile]]\nname='PeliGames'\napplication='FEZ.exe'\nmultiplier=2\n").unwrap();
            let settings = br#"{"backend":"mako","enabled":true,"overlay_enabled":true,"multiplier":2,"flow_scale":1.0,"target_fps":60,"dll_path":"/unchanged/Lossless.dll"}"#;
            fs::write(config.with_extension("json"), settings).unwrap();
            for path in [&config, &config.with_extension("json")] {
                fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Self { directory, config }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.directory);
        }
    }
    #[test]
    fn a_legacy_profile_or_disabled_overlay_cannot_start_controls() {
        let fixture = Fixture::new();
        let path = fixture.config.with_extension("json");
        let mut settings: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        settings.as_object_mut().unwrap().remove("overlay_enabled");
        fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(initial(&fixture.config).is_err());
        settings["overlay_enabled"] = false.into();
        fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(initial(&fixture.config).is_err());
        settings["overlay_enabled"] = true.into();
        fs::write(&path, serde_json::to_vec(&settings).unwrap()).unwrap();
        assert!(initial(&fixture.config).is_ok());
    }
    #[test]
    fn reject_non_finite_or_unbounded_controls() {
        let mut c = Controls {
            multiplier: 2,
            flow_scale: 1.,
            performance: false,
            adaptive: false,
            target_fps: 60,
            paused: false,
            ..Controls::default()
        };
        assert!(c.validate().is_ok());
        c.flow_scale = f32::NAN;
        assert!(c.validate().is_err());
        c.flow_scale = 1.;
        c.multiplier = 6;
        assert!(c.validate().is_err());
    }
    #[test]
    fn live_edits_preserve_dll_startup_identity_and_other_profiles() {
        let fixture = Fixture::new();
        let startup = fs::read(fixture.config.with_extension("json")).unwrap();
        let mut controls = initial(&fixture.config).unwrap();
        controls.multiplier = 4;
        controls.paused = true;
        save(&fixture.config, &controls).unwrap();
        let document: toml::Value = fs::read_to_string(&fixture.config)
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(
            document["global"]["dll"].as_str(),
            Some("/unchanged/Lossless.dll")
        );
        assert_eq!(document["global"]["allow_fp16"].as_bool(), Some(true));
        assert_eq!(document["profile"][0]["multiplier"].as_integer(), Some(3));
        assert_eq!(
            document["profile"][1]["application"].as_str(),
            Some("FEZ.exe")
        );
        assert_eq!(
            document["profile"][1]["frame_generation_enabled"].as_bool(),
            Some(false)
        );
        assert_eq!(
            fs::read(fixture.config.with_extension("json")).unwrap(),
            startup
        );
        let live = initial(&fixture.config).unwrap();
        assert_eq!(live.multiplier, 4);
        assert!(live.paused);
    }
    #[test]
    fn new_live_controls_preserve_startup_fields_and_ignore_stale_cache() {
        let fixture=Fixture::new();
        let startup=fs::read(fixture.config.with_extension("json")).unwrap();
        let mut text=fs::read_to_string(&fixture.config).unwrap();
        text.push_str("gpu='0000:01:00.0'\nultra_performance=true\nscaling_enabled=true\nscaling_factor=1.5\nscaling_method='ls1'\nscaling_sharpness=0.8\n");
        fs::write(&fixture.config,text).unwrap();
        let mut controls=initial(&fixture.config).unwrap();
        assert!(controls.scaling_enabled && controls.ultra_performance);
        controls.base_fps_cap=72;controls.refresh_threshold=120;
        controls.scaling_method="mako".into();controls.scaling_sharpness=0.6;controls.paused=true;
        save(&fixture.config,&controls).unwrap();
        let document:toml::Value=fs::read_to_string(&fixture.config).unwrap().parse().unwrap();
        let profile=&document["profile"][1];
        assert_eq!(profile["base_fps_cap"].as_integer(),Some(72));
        assert_eq!(profile["frame_generation_refresh_threshold"].as_integer(),Some(120));
        assert_eq!(profile["gpu"].as_str(),Some("0000:01:00.0"));
        assert_eq!(profile["scaling_factor"].as_float(),Some(1.5));
        assert_eq!(profile["ultra_performance"].as_bool(),Some(true));
        assert_eq!(profile["scaling_method"].as_str(),Some("mako"));
        assert_eq!(fs::read(fixture.config.with_extension("json")).unwrap(),startup);
        fs::write(fixture.config.with_extension("live.json"),serde_json::to_vec(&Controls::default()).unwrap()).unwrap();
        let reread=initial(&fixture.config).unwrap();
        assert_eq!(reread.base_fps_cap,72);assert_eq!(reread.refresh_threshold,120);assert!(reread.paused);
        assert_eq!(reread.scaling_method,"mako");
    }
    #[test]
    fn live_scale_needs_startup_enablement_and_values_are_bounded() {
        let fixture=Fixture::new();
        let before=fs::read(&fixture.config).unwrap();
        let mut controls=initial(&fixture.config).unwrap();
        controls.scaling_enabled=true;controls.scaling_method="mako".into();
        assert!(save(&fixture.config,&controls).is_err());
        assert_eq!(fs::read(&fixture.config).unwrap(),before);
        controls=Controls::default();controls.base_fps_cap=1001;assert!(controls.validate().is_err());
        controls=Controls::default();controls.refresh_threshold=1001;assert!(controls.validate().is_err());
        controls=Controls::default();controls.scaling_sharpness=f32::NAN;assert!(controls.validate().is_err());
        controls=Controls::default();controls.scaling_method="unknown".into();assert!(controls.validate().is_err());
        let old:Controls=serde_json::from_str(r#"{"multiplier":2,"flow_scale":1.0,"performance":false,"adaptive":false,"target_fps":60,"paused":false}"#).unwrap();
        assert_eq!(old.base_fps_cap,0);assert_eq!(old.scaling_method,"native");
    }
    #[test]
    fn unknown_mako_format_cannot_be_overwritten() {
        let fixture = Fixture::new();
        let controls = initial(&fixture.config).unwrap();
        let unknown = b"version=3\n[[profile]]\nname='PeliGames'\n";
        fs::write(&fixture.config, unknown).unwrap();
        assert!(save(&fixture.config, &controls).is_err());
        assert_eq!(fs::read(&fixture.config).unwrap(), unknown);
        assert!(!fixture.config.with_extension("live.json").exists());
    }
}
