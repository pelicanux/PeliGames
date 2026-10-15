//! Root-side verified staging for the official MangoHUD installer:
//! `peligames --mangohud-install <package> <sha256> <size>`, started through
//! pkexec. It runs before anything else of the launcher (no window, no user
//! configuration), copies the package into a root-owned folder, checks its
//! size and SHA-256 there, unpacks it safely and runs the official installer.
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub const FLAG: &str = "--mangohud-install";
const LIMIT: u64 = 256 * 1024 * 1024;
const MAX_ENTRIES: usize = 2000;
const INSTALLED: [&str; 3] = ["usr/bin/mangohud", "usr/lib/mangohud/lib64/libMangoHud.so", "usr/lib/mangohud/lib32/libMangoHud.so"];

/// Runs the helper and returns the process exit code.
pub fn run(args: &[String]) -> i32 {
    match install(args) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("PeliGames: {error}");
            1
        }
    }
}

fn install(args: &[String]) -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("É necessário autorizar a instalação como administrador.".into());
    }
    let [archive, digest, size] = args else { return Err("Argumentos inválidos.".into()) };
    if !valid_digest(digest) {
        return Err("Checksum inválido.".into());
    }
    let size: u64 = size.parse().map_err(|_| "Tamanho inválido.")?;
    if size == 0 || size > LIMIT {
        return Err("Tamanho inválido.".into());
    }
    let stage = Stage::new()?;
    let (script, expected) = prepare(Path::new(archive), digest, size, &stage.0)?;
    let home = stage.0.join("home");
    fs::create_dir(&home).map_err(|e| e.to_string())?;
    let status = Command::new("/bin/bash")
        .arg(&script)
        .arg("install")
        .current_dir(script.parent().ok_or("Instalador inválido.")?)
        .env_clear()
        .env("PATH", "/usr/sbin:/usr/bin:/sbin:/bin")
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("LANG", "C.UTF-8")
        .status()
        .map_err(|_| "Não foi possível executar o instalador oficial.")?;
    if !status.success() {
        return Err("O instalador oficial não concluiu a instalação.".into());
    }
    for (file, checksum) in expected {
        if sha256_file(&file).ok().as_deref() != Some(checksum.as_str()) {
            return Err("O instalador não instalou todos os arquivos esperados.".into());
        }
    }
    let executable = fs::metadata("/usr/bin/mangohud").is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
    if !executable {
        return Err("O executável MangoHUD não ficou disponível.".into());
    }
    Ok(())
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// A private folder in /tmp, removed when the helper ends.
struct Stage(PathBuf);
impl Stage {
    fn new() -> Result<Self, String> {
        let path = PathBuf::from(format!("/tmp/peligames-mangohud-install-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).map_err(|e| e.to_string())?;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
        Ok(Stage(path))
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hash.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

/// Copies and verifies the package, unpacks it and returns the official
/// installer with the files it must install and their checksums.
fn prepare(archive: &Path, digest: &str, size: u64, stage: &Path) -> Result<(PathBuf, Vec<(PathBuf, String)>), String> {
    let copied = stage.join("official.tar.gz");
    // Copy into a root-owned directory before verification or execution.
    let mut source = fs::File::open(archive).map_err(|_| "Pacote baixado não encontrado.")?;
    let mut output = fs::OpenOptions::new().write(true).create_new(true).open(&copied).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut count = 0u64;
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        let read = source.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        count += read as u64;
        if count > size || count > LIMIT {
            return Err("Tamanho do pacote inválido.".into());
        }
        hash.update(&buffer[..read]);
        output.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
    }
    drop(output);
    if count != size || format!("{:x}", hash.finalize()) != digest {
        return Err("A integridade do pacote não pôde ser confirmada.".into());
    }
    let extracted = stage.join("extracted");
    fs::create_dir(&extracted).map_err(|e| e.to_string())?;
    extract_package(&copied, &extracted)?;
    let mut scripts = Vec::new();
    find(&extracted, "mangohud-setup.sh", &mut scripts);
    let [script] = scripts.as_slice() else { return Err("Instalador oficial não encontrado no pacote.".into()) };
    let folder = script.parent().ok_or("Instalador inválido.")?.to_path_buf();
    let inner = folder.join("MangoHud-package.tar");
    if !inner.is_file() {
        return Err("Conteúdo de instalação não encontrado.".into());
    }
    // The upstream root branch expects this payload to have already been unpacked.
    extract_package(&inner, &folder)?;
    let mut expected = Vec::new();
    for relative in INSTALLED {
        let file = folder.join(relative);
        if !file.is_file() {
            return Err("Pacote oficial incompleto.".into());
        }
        expected.push((Path::new("/").join(relative), sha256_file(&file).map_err(|e| e.to_string())?));
    }
    Ok((script.clone(), expected))
}

fn find(folder: &Path, name: &str, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else { return };
    for entry in entries.flatten() {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            find(&entry.path(), name, found);
        } else if kind.is_file() && entry.file_name() == name {
            found.push(entry.path());
        }
    }
}

/// Unpacks a tar archive (plain or gzip): only regular files and folders,
/// relative paths without `..`, at most 2000 entries and 256 MiB.
fn extract_package(archive: &Path, destination: &Path) -> Result<(), String> {
    let mut file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut magic = [0u8; 2];
    let gzip = file.read(&mut magic).map_err(|e| e.to_string())? == 2 && magic == [0x1f, 0x8b];
    drop(file);
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let reader: Box<dyn Read> = if gzip { Box::new(flate2::read::GzDecoder::new(file)) } else { Box::new(file) };
    let mut package = tar::Archive::new(reader);
    let mut total = 0u64;
    for (index, entry) in package.entries().map_err(|_| "Pacote inválido.")?.enumerate() {
        if index >= MAX_ENTRIES {
            return Err("Pacote contém arquivos demais.".into());
        }
        let mut entry = entry.map_err(|_| "Pacote inválido.")?;
        let kind = entry.header().entry_type();
        let relative = entry.path().map_err(|_| "Pacote inválido.")?.into_owned();
        let safe = relative.components().all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
        if !safe || !(kind.is_dir() || kind.is_file()) {
            return Err("Pacote contém um caminho ou tipo de arquivo inseguro.".into());
        }
        total += entry.size();
        if total > LIMIT {
            return Err("Pacote maior que o permitido.".into());
        }
        let target = destination.join(&relative);
        if kind.is_dir() {
            fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let mut output = fs::File::create(&target).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut output).map_err(|e| e.to_string())?;
            fs::set_permissions(&target, fs::Permissions::from_mode(0o644)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let path = std::env::temp_dir().join(format!("peligames-mangohud-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        path
    }
    fn tar_with(entries: &[(&str, &[u8])], gzip: bool) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o755);
            header.set_entry_type(tar::EntryType::Regular);
            // Written raw so tests can produce the unsafe names the checks refuse.
            header.as_old_mut().name[..name.len()].copy_from_slice(name.as_bytes());
            header.set_cksum();
            builder.append(&header, *data).unwrap();
        }
        let bytes = builder.into_inner().unwrap();
        if !gzip {
            return bytes;
        }
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&bytes).unwrap();
        encoder.finish().unwrap()
    }
    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }
    fn official_package() -> Vec<u8> {
        let payload = tar_with(&[
            ("usr/bin/mangohud", b"#!/bin/sh\n"),
            ("usr/lib/mangohud/lib64/libMangoHud.so", b"lib64"),
            ("usr/lib/mangohud/lib32/libMangoHud.so", b"lib32"),
        ], false);
        tar_with(&[("MangoHud/mangohud-setup.sh", b"#!/bin/bash\n"), ("MangoHud/MangoHud-package.tar", &payload)], true)
    }

    #[test]
    fn checksums_must_be_lowercase_sha256() {
        assert!(valid_digest(&"a".repeat(64)));
        for bad in ["a".repeat(63), "A".repeat(64), "g".repeat(64), "../".repeat(21) + "a"] {
            assert!(!valid_digest(&bad), "{bad}");
        }
    }

    #[test]
    fn verified_package_is_unpacked_with_installer_and_expected_files() {
        let dir = scratch();
        let package = official_package();
        let archive = dir.join("download.tar.gz");
        fs::write(&archive, &package).unwrap();
        let stage = dir.join("stage");
        fs::create_dir(&stage).unwrap();
        let (script, expected) = prepare(&archive, &digest(&package), package.len() as u64, &stage).unwrap();
        assert!(script.ends_with("MangoHud/mangohud-setup.sh"));
        let installed: Vec<_> = expected.iter().map(|(path, _)| path.to_string_lossy().into_owned()).collect();
        assert_eq!(installed, ["/usr/bin/mangohud", "/usr/lib/mangohud/lib64/libMangoHud.so", "/usr/lib/mangohud/lib32/libMangoHud.so"]);
        assert_eq!(expected[1].1, digest(b"lib64"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn wrong_size_or_checksum_never_unpacks() {
        let dir = scratch();
        let package = official_package();
        let archive = dir.join("download.tar.gz");
        fs::write(&archive, &package).unwrap();
        for (sum, size) in [("b".repeat(64), package.len() as u64), (digest(&package), package.len() as u64 - 1), (digest(&package), package.len() as u64 + 1)] {
            let stage = scratch();
            assert!(prepare(&archive, &sum, size, &stage).is_err());
            assert!(!stage.join("extracted").exists(), "nada é extraído antes da verificação");
            let _ = fs::remove_dir_all(stage);
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn unsafe_paths_links_and_missing_installer_are_refused() {
        let dir = scratch();
        for name in ["../escape", "/etc/passwd", "a/../../b"] {
            let archive = dir.join("bad.tar");
            fs::write(&archive, tar_with(&[(name, b"x")], false)).unwrap();
            let target = scratch();
            assert!(extract_package(&archive, &target).unwrap_err().contains("inseguro"), "{name}");
            let _ = fs::remove_dir_all(target);
        }
        let mut builder = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        builder.append_link(&mut header, "link", "/etc/shadow").unwrap();
        let archive = dir.join("link.tar");
        fs::write(&archive, builder.into_inner().unwrap()).unwrap();
        let target = scratch();
        assert!(extract_package(&archive, &target).is_err());
        assert!(fs::read_dir(&target).unwrap().next().is_none(), "nenhum link criado");
        let _ = fs::remove_dir_all(target);

        let package = tar_with(&[("MangoHud/readme.txt", b"no installer")], true);
        let archive = dir.join("no-installer.tar.gz");
        fs::write(&archive, &package).unwrap();
        let stage = scratch();
        assert!(prepare(&archive, &digest(&package), package.len() as u64, &stage).unwrap_err().contains("não encontrado"));
        let _ = fs::remove_dir_all(stage);
        let _ = fs::remove_dir_all(dir);
    }

    /// Checks and unpacks a real official package given in PELIGAMES_MANGOHUD_PACKAGE
    /// (path, SHA-256 and size separated by spaces); nothing is installed.
    #[test]
    #[ignore = "needs a downloaded official MangoHUD package"]
    fn official_package_is_verified_and_unpacked() {
        let spec = std::env::var("PELIGAMES_MANGOHUD_PACKAGE").unwrap();
        let [path, sum, size] = spec.split(' ').collect::<Vec<_>>()[..] else { panic!("path sha256 size") };
        let stage = scratch();
        let (script, expected) = prepare(Path::new(path), sum, size.parse().unwrap(), &stage).unwrap();
        println!("installer: {}", script.display());
        for (file, sum) in &expected { println!("{} {sum}", file.display()); }
        let _ = fs::remove_dir_all(stage);
    }
    #[test]
    fn helper_refuses_to_run_without_root() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }
        assert_eq!(run(&["x".into(), "a".repeat(64), "1".into()]), 1);
    }
}
