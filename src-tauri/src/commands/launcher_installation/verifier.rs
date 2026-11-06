//! Execute the bundled verifier from a sealed, anonymous Linux file.
//! No system installation, PATH lookup, persistent extraction or root is involved.
use std::path::Path;
#[cfg(target_os = "linux")]
use std::{
    fs::File,
    io::{self, Write},
    os::fd::{AsRawFd, FromRawFd},
    process::Command,
};

pub struct Verifier {
    #[cfg(target_os = "linux")]
    executable: File,
}

#[cfg(target_os = "linux")]
impl Verifier {
    pub fn command(&self) -> Command {
        // The parent retains this sealed file through the child's entire execution.
        let mut command = Command::new(format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            self.executable.as_raw_fd()
        ));
        command
            .env_remove("LD_LIBRARY_PATH")
            .env_remove("LD_PRELOAD")
            .env_remove("LD_AUDIT")
            .env("OPENSSL_CONF", "/dev/null")
            .env_remove("OPENSSL_MODULES");
        command
    }
}

impl Verifier {
    pub fn verify(&self, id: &str, path: &Path) -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            verify_signature(self, id, path)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (id, path);
            Err("A instalação automática de launchers está disponível somente no Linux.".into())
        }
    }
}

#[cfg(target_os = "linux")]
fn normalized(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
#[cfg(target_os = "linux")]
fn publisher_matches(id: &str, output: &str) -> bool {
    // -index 0 restricts verification to the same signature whose leaf is inspected.
    let section = match output.split("Signer's certificate:").nth(1) {
        Some(s) => s,
        None => return false,
    };
    let subject = match section
        .lines()
        .find_map(|line| line.trim().strip_prefix("Subject:"))
    {
        Some(s) => s,
        None => return false,
    };
    let organization = if subject.contains("/O=") {
        subject
            .split("/O=")
            .nth(1)
            .and_then(|s| s.split('/').next())
            .map(normalized)
            .unwrap_or_default()
    } else {
        // Current osslsigncode prints RFC2253 names; commas in O are escaped.
        let mut attributes = Vec::new();
        let mut field = String::new();
        let mut escaped = false;
        for c in subject.trim().chars() {
            if escaped {
                field.push(c);
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == ',' || c == '+' {
                attributes.push(std::mem::take(&mut field));
            } else {
                field.push(c);
            }
        }
        attributes.push(field);
        let organizations: Vec<_> = attributes
            .iter()
            .filter_map(|field| field.trim().strip_prefix("O="))
            .collect();
        if organizations.len() != 1 {
            return false;
        }
        normalized(organizations[0])
    };
    let expected: &[&str] = match id {
        "ubisoft-connect" => &["Ubisoft Entertainment", "Ubisoft Entertainment SA"],
        "ea-app" => &["Electronic Arts Inc.", "Electronic Arts, Inc."],
        "battle-net" => &[
            "Blizzard Entertainment, Inc.",
            "Blizzard Entertainment Inc.",
        ],
        _ => &[],
    };
    expected.iter().any(|name| organization == normalized(name))
}
#[cfg(target_os = "linux")]
fn verify_signature(verifier: &Verifier, id: &str, path: &Path) -> Result<(), String> {
    let ca = [
        "/etc/ssl/certs/ca-certificates.crt",
        "/etc/pki/tls/certs/ca-bundle.crt",
        "/etc/ssl/ca-bundle.pem",
    ]
    .into_iter()
    .find(|p| Path::new(p).is_file())
    .ok_or("Pacote de certificados CA não encontrado no sistema.")?;
    let output = verifier
        .command()
        .args([
            "verify",
            "-index",
            "0",
            "-CAfile",
            ca,
            "-TSA-CAfile",
            ca,
            "-in",
        ])
        .arg(path)
        .output()
        .map_err(|e| format!("Não foi possível verificar a assinatura do instalador: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    if !output.status.success() || !publisher_matches(id, &text) {
        return Err("Não foi possível confirmar a assinatura e o fornecedor do instalador. Nenhum executável foi iniciado.".into());
    }
    Ok(())
}
pub fn prepare() -> Result<Verifier, String> {
    #[cfg(target_os = "linux")]
    {
        let payload = include_bytes!(env!("PELIGAMES_INSTALLER_VERIFIER_EMBEDDED"));
        let mut decoder = flate2::read::GzDecoder::new(payload.as_slice());
        let flags = libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING;
        let mut fd = unsafe {
            libc::memfd_create(
                c"peligames-installer-verifier".as_ptr(),
                flags | libc::MFD_EXEC,
            )
        };
        // MFD_EXEC is required by hardened kernels; older kernels predate the flag.
        if fd < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINVAL) {
            fd = unsafe { libc::memfd_create(c"peligames-installer-verifier".as_ptr(), flags) };
        }
        if fd < 0 {
            return Err(format!(
                "Não foi possível preparar o verificador interno: {}",
                io::Error::last_os_error()
            ));
        }
        let mut executable = unsafe { File::from_raw_fd(fd) };
        io::copy(&mut decoder, &mut executable)
            .map_err(|e| format!("Verificador interno inválido: {e}"))?;
        executable.flush().map_err(|e| e.to_string())?;
        let seals =
            libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;
        if unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) } < 0 {
            return Err(format!(
                "Não foi possível proteger o verificador interno: {}",
                io::Error::last_os_error()
            ));
        }
        let verifier = Verifier { executable };
        let version = verifier
            .command()
            .arg("--version")
            .output()
            .map_err(|e| format!("Não foi possível iniciar o verificador interno: {e}"))?;
        if !version.status.success()
            || !String::from_utf8_lossy(&version.stdout).contains(concat!(
                "osslsigncode ",
                env!("PELIGAMES_INSTALLER_VERIFIER_VERSION")
            ))
        {
            return Err(
                "O verificador interno não está disponível nesta compilação do PeliGames.".into(),
            );
        }
        Ok(verifier)
    }
    #[cfg(not(target_os = "linux"))]
    Err("A instalação automática de launchers está disponível somente no Linux.".into())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    #[test]
    fn binds_leaf_publisher() {
        assert!(publisher_matches("ea-app", "Signer's certificate:\n Subject: /C=US/O=Electronic Arts, Inc./CN=Electronic Arts, Inc.\n"));
        assert!(publisher_matches("ea-app", "Signer's certificate:\n Subject: CN=Electronic Arts\\, Inc.,O=Electronic Arts\\, Inc.,C=US\n"));
        assert!(!publisher_matches(
            "ea-app",
            "Signer's certificate:\n Subject: /O=Attacker/CN=Electronic Arts, Inc.\n"
        ));
    }
    #[test]
    fn bundled_verifier_runs_without_system_tools_and_cannot_be_modified() {
        let mut verifier = prepare().unwrap();
        let output = verifier
            .command()
            .env("PATH", "/nonexistent")
            .arg("--version")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("osslsigncode"));
        assert!(verifier.executable.write_all(b"changed").is_err());
        let path = format!(
            "/proc/{}/fd/{}",
            std::process::id(),
            verifier.executable.as_raw_fd()
        );
        drop(verifier);
        assert!(!std::path::Path::new(&path).exists());
    }
    #[test]
    fn unsigned_executable_is_rejected_without_an_external_verifier() {
        let verifier = prepare().unwrap();
        let output = verifier
            .command()
            .env("PATH", "/nonexistent")
            .args(["verify", "-in", "/proc/self/exe"])
            .output()
            .unwrap();
        assert!(!output.status.success());
    }
}
