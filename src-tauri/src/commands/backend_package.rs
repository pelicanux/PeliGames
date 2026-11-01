//! Prepare complete backend downloads before replacing the installed files.
use flate2::read::GzDecoder;
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};
use tar::Archive;

pub(super) struct UpdateStage {
    root: PathBuf,
    keep: bool,
}
impl UpdateStage {
    pub fn new(parent: &Path, arch: &str) -> Result<Self, String> {
        if !matches!(arch, "rdna3" | "rdna4") {
            return Err("Arquitetura de backend inválida.".into());
        }
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        for number in 0..1000 {
            let suffix = if number == 0 {
                String::new()
            } else {
                format!(" ({number})")
            };
            let root = parent.join(format!(".{arch}-update{suffix}"));
            match fs::create_dir(&root) {
                Ok(()) => return Ok(Self { root, keep: false }),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e.to_string()),
            }
        }
        Err("Não foi possível preparar a atualização do backend.".into())
    }
    pub fn unpack(&self, name: &str, bits: &str, bytes: &[u8]) -> Result<(), String> {
        if !matches!(bits, "32" | "64") {
            return Err("Arquitetura de pacote inválida.".into());
        }
        let zip = name.to_ascii_lowercase().ends_with(".zip");
        if !(if zip {
            bytes.starts_with(b"PK\x03\x04")
        } else {
            bytes.starts_with(b"\x1f\x8b\x08")
        }) {
            return Err(format!("O download de {name} não é um arquivo de backend válido. Os arquivos instalados foram preservados."));
        }
        let directory = self.root.join("download").join(bits);
        if directory.exists() {
            return Err(format!("Pacote de {bits} bits duplicado."));
        }
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        let cursor = Cursor::new(bytes);
        if zip {
            zip::ZipArchive::new(cursor)
                .map_err(|e| e.to_string())?
                .extract(&directory)
                .map_err(|e| e.to_string())?;
        } else {
            Archive::new(GzDecoder::new(cursor))
                .unpack(&directory)
                .map_err(|e| format!("Falha ao extrair {name}: {e}"))?;
        }
        if !ready(&directory) {
            return Err(format!("O pacote {name} está incompleto: faltam install.sh, model-tools/extract_model.sh ou dlssnr-amd. O backend instalado foi preservado."));
        }
        Ok(())
    }
    pub fn publish(
        mut self,
        target: &Path,
        version: &str,
        cancelled: impl Fn() -> bool,
    ) -> Result<(), String> {
        if target.parent() != self.root.parent()
            || fs::symlink_metadata(target).is_ok_and(|m| m.file_type().is_symlink())
        {
            return Err("Destino inválido para atualização do backend.".into());
        }
        let downloads = self.root.join("download");
        if !downloads.is_dir() {
            return Err("Nenhum pacote de backend foi preparado.".into());
        }
        let candidate = self.root.join("candidate");
        // Keep already extracted user models; release packages intentionally contain no weights.
        if target.exists() {
            super::copy_dir_all(target, &candidate).map_err(|e| e.to_string())?;
        }
        super::copy_dir_all(&downloads, &candidate).map_err(|e| e.to_string())?;
        fs::write(candidate.join("version.txt"), version).map_err(|e| e.to_string())?;
        if cancelled() {
            return Err("Download cancelado pelo usuário.".into());
        }
        let previous = self.root.join("previous");
        let replacing = target.exists();
        if replacing {
            fs::rename(target, &previous).map_err(|e| e.to_string())?;
        }
        if let Err(error) = fs::rename(&candidate, target) {
            if replacing {
                if let Err(restore) = fs::rename(&previous, target) {
                    self.keep = true;
                    return Err(format!("Falha ao aplicar o backend: {error}. A cópia anterior foi preservada em {}: {restore}",previous.display()));
                }
            }
            return Err(error.to_string());
        }
        Ok(())
    }
}
impl Drop for UpdateStage {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}
fn ready(path: &Path) -> bool {
    path.join("install.sh").is_file()
        && path.join("model-tools/extract_model.sh").is_file()
        && path.join("dlssnr-amd").is_dir()
}

pub(super) fn verify_download(bytes: &[u8], checksum: Option<&str>) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    if let Some(expected) = checksum {
        if expected.len() != 64
            || !expected.bytes().all(|b| b.is_ascii_hexdigit())
            || !format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(expected)
        {
            return Err("O checksum do pacote de backend não confere. Os arquivos instalados foram preservados.".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "peligames-backend-package-test-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn published_checksums_reject_changed_or_invalid_downloads() {
        assert!(verify_download(
            b"abc",
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        )
        .is_ok());
        assert!(verify_download(
            b"changed",
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        )
        .is_err());
        assert!(verify_download(b"abc", Some("invalid")).is_err());
        assert!(verify_download(b"abc", None).is_ok());
    }
    fn package() -> Vec<u8> {
        let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for name in [
            "install.sh",
            "model-tools/extract_model.sh",
            "dlssnr-amd/shaders/test.spv",
        ] {
            let data = b"new release";
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o755);
            h.set_cksum();
            tar.append_data(&mut h, name, data.as_slice()).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap()
    }
    #[test]
    fn html_or_failed_second_package_leaves_installed_backend_untouched() {
        let fixture = Fixture::new();
        let target = fixture.0.join("rdna4");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("version.txt"), "old").unwrap();
        {
            let stage = UpdateStage::new(&fixture.0, "rdna4").unwrap();
            stage
                .unpack("linux-x86_64.tar.gz", "64", &package())
                .unwrap();
            assert!(stage
                .unpack("linux-i686.tar.gz", "32", b"<html>Sign in</html>")
                .is_err());
            assert_eq!(
                fs::read_to_string(target.join("version.txt")).unwrap(),
                "old"
            );
        }
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn complete_update_preserves_extracted_models_and_publishes_version_together() {
        let fixture = Fixture::new();
        let target = fixture.0.join("rdna4");
        fs::create_dir_all(target.join("64/dlssnr-amd")).unwrap();
        fs::write(target.join("64/dlssnr-amd/dlssnr.bin"), "user model").unwrap();
        fs::write(target.join("64/install.sh"), "old script").unwrap();
        let stage = UpdateStage::new(&fixture.0, "rdna4").unwrap();
        for bits in ["64", "32"] {
            stage.unpack("linux.tar.gz", bits, &package()).unwrap();
        }
        stage.publish(&target, "v0.0.8.1-rdna4", || false).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("version.txt")).unwrap(),
            "v0.0.8.1-rdna4"
        );
        assert_eq!(
            fs::read_to_string(target.join("64/dlssnr-amd/dlssnr.bin")).unwrap(),
            "user model"
        );
        assert_eq!(
            fs::read_to_string(target.join("64/install.sh")).unwrap(),
            "new release"
        );
        assert!(ready(&target.join("32")));
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn cancellation_before_publish_preserves_current_version() {
        let fixture = Fixture::new();
        let target = fixture.0.join("rdna4");
        fs::create_dir(&target).unwrap();
        fs::write(target.join("version.txt"), "old").unwrap();
        let stage = UpdateStage::new(&fixture.0, "rdna4").unwrap();
        stage.unpack("linux.tar.gz", "64", &package()).unwrap();
        assert!(stage.publish(&target, "new", || true).is_err());
        assert_eq!(
            fs::read_to_string(target.join("version.txt")).unwrap(),
            "old"
        );
        assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 1);
    }

    #[test]
    fn incomplete_zip_is_rejected_before_replacing_a_backend() {
        use std::io::Write;
        let fixture = Fixture::new();
        let stage = UpdateStage::new(&fixture.0, "rdna4").unwrap();
        let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
        z.start_file("README.txt", zip::write::FileOptions::default())
            .unwrap();
        z.write_all(b"source snapshot").unwrap();
        let bytes = z.finish().unwrap().into_inner();
        assert!(stage.unpack("linux.zip", "64", &bytes).is_err());
        assert!(!fixture.0.join("rdna4").exists());
    }
}
