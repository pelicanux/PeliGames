use super::*;
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::{io::Cursor, os::unix::fs::PermissionsExt, time::Duration};
const SHA256: &str = "a0b7c1ed1a055ebdcfbb74d00104d292ea4da3106cf78b6b6fd48865ca536a29";
const ARCHIVE_ROOT: &str = "AMDNR-Linux-v0.3.6";
const URL: &str = "https://github.com/hassanm0301/AMDNR-Linux/releases/download/linux-v0.3.6/AMDNR-Linux-v0.3.6.tar.gz";
pub(super) fn ready(base: &Path) -> bool {
    let root = component(base);
    fs::read_to_string(root.join("verified.sha256")).is_ok_and(|s| s == SHA256)
        && [
            "linux/manage.py",
            "linux/setup.py",
            "linux/runtimes.py",
            "linux/build/x86_64-unix/amdhip64_7.so",
        ]
        .iter()
        .all(|name| root.join(name).is_file())
}
fn unpack(bytes: &[u8], stage: &Path) -> Result<(), String> {
    let mut archive = tar::Archive::new(GzDecoder::new(Cursor::new(bytes)));
    let mut total = 0u64;
    for (count, entry) in archive.entries().map_err(|e| e.to_string())?.enumerate() {
        let mut entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path().map_err(|e| e.to_string())?.into_owned();
        if count > 10_000
            || !path.starts_with(ARCHIVE_ROOT)
            || path
                .components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
        {
            return Err("Arquivo da ponte contém um caminho inválido.".into());
        }
        let kind = entry.header().entry_type();
        if !kind.is_file() && !kind.is_dir() {
            return Err("Arquivo da ponte contém um link ou entrada especial.".into());
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Arquivo grande demais.")?;
        if total > 192 * 1024 * 1024 {
            return Err("Arquivo da ponte excede o limite de extração.".into());
        }
        entry.unpack_in(stage).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub async fn download(
    base: &Path,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    if ready(base) {
        return Ok(());
    }
    progress("Baixando a ponte AMDNR-Linux verificada…".into());
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;
    let mut response = client
        .get(URL)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| e.to_string())? {
        if bytes.len() + chunk.len() > 16 * 1024 * 1024 {
            return Err("Download da ponte excedeu o tamanho esperado.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    if format!("{:x}", Sha256::digest(&bytes)) != SHA256 {
        return Err("A integridade da ponte AMDNR-Linux não confere.".into());
    }
    progress("Verificando e extraindo a ponte…".into());
    let base = base.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let stage = base.join("extracting-bridge");
        if stage.exists() { fs::remove_dir_all(&stage).map_err(|e| e.to_string())?; }
        fs::create_dir(&stage).map_err(|e| e.to_string())?;
        let result = (|| {
            unpack(&bytes, &stage)?;
            let target = component(&base);
            if target.exists() { return Err("A pasta da ponte existe, mas não está verificada. Preserve-a e remova-a antes de tentar novamente.".into()); }
            let extracted = stage.join(ARCHIVE_ROOT);
            fs::write(extracted.join("verified.sha256"), SHA256).map_err(|e| e.to_string())?;
            fs::set_permissions(&extracted, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
            fs::rename(&extracted, &target).map_err(|e| e.to_string())?;
            if !ready(&base) { return Err("A ponte extraída está incompleta.".into()); }
            Ok(())
        })();
        let _ = fs::remove_dir_all(stage);
        result
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};
    #[test]
    fn refuses_links_and_excessive_archive_entries() {
        let stage =
            std::env::temp_dir().join(format!("peligames-daniel-archive-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&stage).unwrap();
        let mut tar = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_link_name("/etc/passwd").unwrap();
        header.set_cksum();
        tar.append_data(
            &mut header,
            format!("{ARCHIVE_ROOT}/leak"),
            std::io::empty(),
        )
        .unwrap();
        let bytes = tar.into_inner().unwrap().finish().unwrap();
        assert!(unpack(&bytes, &stage).unwrap_err().contains("link"));
        assert!(!stage.join(ARCHIVE_ROOT).join("leak").exists());
        let _ = fs::remove_dir_all(stage);
    }
    #[test]
    #[ignore = "requires the pinned release archive in PELIGAMES_DANIEL_ARCHIVE; does not download"]
    fn pinned_upstream_archive_installs_and_restores_isolated_fixture() {
        let bytes =
            fs::read(std::env::var("PELIGAMES_DANIEL_ARCHIVE").expect("fixture archive path"))
                .unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), SHA256);
        let stage = std::env::temp_dir().join(format!(
            "peligames-daniel-real-archive-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&stage).unwrap();
        unpack(&bytes, &stage).unwrap();
        assert!(stage
            .join(ARCHIVE_ROOT)
            .join("linux/bridge/registration.c")
            .is_file());
        assert!(stage.join(ARCHIVE_ROOT).join("LICENSE").is_file());
        assert!(stage
            .join(ARCHIVE_ROOT)
            .join("linux/build/x86_64-windows/amdhip64_7.dll")
            .is_file());
        assert!(!stage
            .join(ARCHIVE_ROOT)
            .join("linux/build/package")
            .exists());
        fs::write(stage.join(ARCHIVE_ROOT).join("verified.sha256"), SHA256).unwrap();
        let package = stage.join("fixture-package");
        let game = stage.join("fixture-game");
        fs::create_dir_all(package.join("amdnr-linux")).unwrap();
        fs::create_dir(&game).unwrap();
        fs::write(package.join("winmm.dll"), "test proxy").unwrap();
        fs::write(package.join("d3d12.dll"), "test replacement").unwrap();
        fs::write(package.join("amdnr-linux/run.sh"), "#!/bin/sh\nexit 0\n").unwrap();
        fs::write(game.join("d3d12.dll"), "original game library").unwrap();
        fs::write(game.join("save.dat"), "unrelated save").unwrap();
        super::super::operations::run(
            &stage,
            "manage.py",
            &[
                "install".into(),
                string(&package),
                string(&game),
                "--proxy".into(),
                "winmm.dll".into(),
            ],
            &stage.join("fixture-install.log"),
            &|_| {},
        )
        .unwrap();
        assert!(game.join(JOURNAL).is_file());
        assert_eq!(
            fs::read_to_string(game.join("d3d12.dll")).unwrap(),
            "test replacement"
        );
        super::super::operations::run(
            &stage,
            "manage.py",
            &["uninstall".into(), string(&game)],
            &stage.join("fixture-remove.log"),
            &|_| {},
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(game.join("d3d12.dll")).unwrap(),
            "original game library"
        );
        assert_eq!(
            fs::read_to_string(game.join("save.dat")).unwrap(),
            "unrelated save"
        );
        assert!(!game.join(JOURNAL).exists());
        assert!(!game.join("winmm.dll").exists());

        fs::remove_dir_all(stage).unwrap();
    }
}
