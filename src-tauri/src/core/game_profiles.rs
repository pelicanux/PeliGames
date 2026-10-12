//! Game profiles: a prefix of its own for a game a launcher installed, mirrored
//! from the launcher's prefix without copying it.
//!
//! - Overlay: the launcher's prefix is the read-only lower layer; the profile
//!   keeps only what it changes (`upper`), seen together at `prefix` through
//!   fuse-overlayfs. Works on any filesystem.
//! - Reflink: a copy-on-write copy (Btrfs, XFS, bcachefs) sharing every block
//!   with the original until the profile changes it.
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileKind {
    #[default]
    Overlay,
    Reflink,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GameProfile {
    pub kind: ProfileKind,
    /// The launcher's prefix the profile mirrors.
    pub base: String,
    /// `<launcher folder>/profiles/<id>`: everything of this profile.
    pub root: String,
}

impl GameProfile {
    /// The profile's Wine prefix (the merged view for an overlay).
    pub fn prefix(&self) -> PathBuf {
        Path::new(&self.root).join("prefix")
    }
    /// A path of the launcher's prefix, seen inside this profile.
    pub fn translate(&self, path: &str) -> String {
        match Path::new(path).strip_prefix(&self.base) {
            Ok(rest) => self.prefix().join(rest).to_string_lossy().into_owned(),
            Err(_) => path.to_string(),
        }
    }
}

fn helper(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/local/bin"].iter().map(|p| Path::new(p).join(name)).find(|p| p.is_file())
}

pub fn overlay_available() -> bool {
    fs::OpenOptions::new().read(true).write(true).open("/dev/fuse").is_ok()
        && helper("fuse-overlayfs").is_some()
        && helper("fusermount3").is_some()
}

/// Copy-on-write copies (reflinks) work on Btrfs, XFS and bcachefs.
pub fn reflink_available(path: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStrExt;
        let Ok(name) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false; };
        let mut info: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(name.as_ptr(), &mut info) } != 0 {
            return false;
        }
        const BTRFS: i64 = 0x9123_683E;
        const XFS: i64 = 0x5846_5342;
        const BCACHEFS: i64 = 0xCA45_1A4E;
        [BTRFS, XFS, BCACHEFS].contains(&(info.f_type as i64))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        false
    }
}

fn mounted(path: &Path) -> bool {
    // mountinfo escapes whitespace and backslashes in mountpoint names.
    let escaped = path.to_string_lossy().replace('\\', "\\134").replace(' ', "\\040").replace('\t', "\\011").replace('\n', "\\012");
    fs::read_to_string("/proc/self/mountinfo").unwrap_or_default().lines().any(|line| line.split_whitespace().nth(4) == Some(escaped.as_str()))
}

fn unmount(path: &Path) -> Result<(), String> {
    if !mounted(path) {
        return Ok(());
    }
    let output = Command::new(helper("fusermount3").ok_or("fusermount3 ausente.")?).arg("-u").arg(path).output().map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("Feche o jogo e os programas desse perfil antes: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

/// Makes the profile's prefix usable: mounts an overlay that is not mounted
/// (after a reboot) or whose mount was left behind by a stopped process.
pub fn ensure_ready(profile: &GameProfile) -> Result<(), String> {
    let prefix = profile.prefix();
    match profile.kind {
        ProfileKind::Reflink => {
            if prefix.join("drive_c").is_dir() { Ok(()) } else { Err(format!("Perfil reflink sem prefixo: {}", prefix.display())) }
        }
        ProfileKind::Overlay => {
            if mounted(&prefix) {
                if fs::read_dir(&prefix).is_ok() {
                    return Ok(());
                }
                // A mount whose fuse-overlayfs process ended ("transport endpoint is not connected").
                unmount(&prefix)?;
            }
            mount(profile)
        }
    }
}

fn mount(profile: &GameProfile) -> Result<(), String> {
    if !overlay_available() {
        return Err("Perfis overlay precisam de fuse-overlayfs, fusermount3 e acesso a /dev/fuse.".into());
    }
    let root = Path::new(&profile.root);
    let (upper, work, prefix) = (root.join("upper"), root.join("work"), profile.prefix());
    for path in [&upper, &work, &prefix] {
        fs::create_dir_all(path).map_err(|e| e.to_string())?;
    }
    for path in [Path::new(&profile.base), &upper, &work, &prefix] {
        if path.to_string_lossy().chars().any(|c| [':', ',', '\\', '\n'].contains(&c)) {
            return Err("O caminho do perfil contém caracteres incompatíveis com o overlay (: , \\).".into());
        }
    }
    let options = format!("lowerdir={},upperdir={},workdir={}", profile.base, upper.display(), work.display());
    let output = Command::new(helper("fuse-overlayfs").ok_or("fuse-overlayfs ausente.")?).args(["-o", &options]).arg(&prefix).output().map_err(|e| e.to_string())?;
    if !output.status.success() || !mounted(&prefix) {
        return Err(format!("Não foi possível montar o perfil: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    Ok(())
}

/// Creates a profile of `base` in `profiles/<new id>`.
pub fn create(base: &Path, profiles: &Path, kind: ProfileKind) -> Result<GameProfile, String> {
    if !base.join("drive_c").is_dir() {
        return Err("O prefixo do launcher não foi encontrado.".into());
    }
    fs::create_dir_all(profiles).map_err(|e| e.to_string())?;
    let root = profiles.join(uuid::Uuid::new_v4().to_string());
    let profile = GameProfile { kind, base: base.to_string_lossy().into_owned(), root: root.to_string_lossy().into_owned() };
    let result = match kind {
        ProfileKind::Overlay => mount(&profile),
        ProfileKind::Reflink => {
            if !reflink_available(base) || !reflink_available(profiles) {
                Err("Reflink exige o prefixo e os perfis num disco Btrfs, XFS ou bcachefs.".into())
            } else {
                fs::create_dir_all(&root).map_err(|e| e.to_string()).and_then(|_| {
                    let output = Command::new("cp").args(["-a", "--reflink=always"]).arg(base).arg(profile.prefix()).output().map_err(|e| e.to_string())?;
                    if output.status.success() { Ok(()) } else { Err(format!("Não foi possível criar o perfil reflink: {}", String::from_utf8_lossy(&output.stderr).trim())) }
                })
            }
        }
    };
    if let Err(error) = result {
        let _ = remove(&profile);
        return Err(error);
    }
    Ok(profile)
}

/// Deletes the profile: its changes (overlay) or its copy (reflink). The
/// launcher's prefix is never touched.
pub fn remove(profile: &GameProfile) -> Result<(), String> {
    let root = Path::new(&profile.root);
    // Only a `profiles/<uuid>` folder is ever removed.
    let named = root.file_name().and_then(|n| n.to_str()).is_some_and(|n| uuid::Uuid::parse_str(n).is_ok());
    if !named || root.parent().and_then(|p| p.file_name()).is_none_or(|n| n != "profiles") || root.starts_with(&profile.base) {
        return Err("Pasta de perfil inválida.".into());
    }
    if profile.kind == ProfileKind::Overlay {
        unmount(&profile.prefix())?;
    }
    match fs::symlink_metadata(root) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => fs::remove_dir_all(root).map_err(|e| e.to_string()),
        Ok(_) => Err("Pasta de perfil inválida.".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_move_into_the_profile_and_removal_stays_inside_profiles() {
        let profile = GameProfile { kind: ProfileKind::Reflink, base: "/games/ea/prefix".into(), root: "/games/ea/profiles/00000000-0000-4000-8000-000000000001".into() };
        assert_eq!(profile.translate("/games/ea/prefix/drive_c/Program Files/EA Games/BF/bf.exe"), "/games/ea/profiles/00000000-0000-4000-8000-000000000001/prefix/drive_c/Program Files/EA Games/BF/bf.exe");
        assert_eq!(profile.translate("/elsewhere/x.exe"), "/elsewhere/x.exe");
        let outside = GameProfile { root: "/games/ea/prefix".into(), ..profile.clone() };
        assert!(remove(&outside).is_err(), "nunca apaga fora de profiles/<id>");
    }
    #[test]
    fn reflink_profiles_copy_the_prefix_or_refuse_cleanly() {
        let base = std::env::temp_dir().join(format!("peligames-profile-{}", uuid::Uuid::new_v4()));
        let prefix = base.join("prefix");
        fs::create_dir_all(prefix.join("drive_c/Games")).unwrap();
        fs::write(prefix.join("drive_c/Games/game.exe"), b"MZ").unwrap();
        let profiles = base.join("profiles");
        match create(&prefix, &profiles, ProfileKind::Reflink) {
            // Btrfs/XFS temporary folder: a real profile, removed again.
            Ok(profile) => {
                assert!(profile.prefix().join("drive_c/Games/game.exe").is_file());
                remove(&profile).unwrap();
                assert!(!Path::new(&profile.root).exists());
            }
            // Other filesystems: refused without leftovers.
            Err(_) => assert!(fs::read_dir(&profiles).map(|entries| entries.count()).unwrap_or(0) == 0),
        }
        assert!(prefix.join("drive_c/Games/game.exe").is_file(), "o prefixo do launcher fica intacto");
        fs::remove_dir_all(base).unwrap();
    }
}
