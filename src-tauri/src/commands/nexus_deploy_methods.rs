//! Deployment primitives; no third-party mod-manager code is used here.
use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};
use serde::{Deserialize, Serialize};
use super::{nexus_local::NexusGame, nexus_profile::directory};
#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method { #[default] Copy, Symlink, Hardlink, Vfs }
impl Method {
    pub(super) fn place(self, source: &Path, target: &Path) -> Result<(), String> {
        match self {
            Self::Copy => fs::copy(source, target).map(|_| ()).map_err(|e| e.to_string()),
            Self::Symlink => std::os::unix::fs::symlink(source, target).map_err(|e| e.to_string()),
            Self::Hardlink => fs::hard_link(source, target).map_err(|e| format!("Hardlink exige o mesmo sistema de arquivos: {e}")),
            Self::Vfs => Err("VFS não aplica arquivos físicos.".into()),
        }
    }
}
pub(super) fn backup(source: &Path, target: &Path) -> Result<(), String> {
    if fs::symlink_metadata(source).map_err(|e| e.to_string())?.file_type().is_symlink() {
        std::os::unix::fs::symlink(fs::read_link(source).map_err(|e| e.to_string())?, target).map_err(|e| e.to_string())
    } else {
        fs::hard_link(source, target).or_else(|_| fs::copy(source, target).map(|_| ())).map_err(|e| e.to_string())
    }
}
fn helper(name: &str) -> Option<PathBuf> {
    ["/usr/bin", "/bin", "/usr/local/bin"].iter().map(|p| Path::new(p).join(name)).find(|p| p.is_file())
}
pub(super) fn vfs_available() -> bool {
    fs::OpenOptions::new().read(true).write(true).open("/dev/fuse").is_ok()
        && helper("fuse-overlayfs").is_some() && helper("fusermount3").is_some() && helper("bwrap").is_some()
}
fn state(game: &NexusGame) -> Result<PathBuf, String> { Ok(directory(game)?.join("vfs")) }
fn mounted(path: &Path) -> bool {
    // mountinfo escapes whitespace and backslashes in mountpoint names.
    let escaped = path.to_string_lossy().replace('\\', "\\134").replace(' ', "\\040").replace('\t', "\\011").replace('\n', "\\012");
    fs::read_to_string("/proc/self/mountinfo").unwrap_or_default().lines().any(|line| line.split_whitespace().nth(4) == Some(escaped.as_str()))
}
pub(super) fn unmount(game: &NexusGame) -> Result<(), String> {
    let merged = state(game)?.join("merged");
    if mounted(&merged) {
        let output = Command::new(helper("fusermount3").ok_or("fusermount3 ausente.")?).arg("-u").arg(&merged).output().map_err(|e| e.to_string())?;
        if !output.status.success() { return Err(format!("Feche os processos que utilizam o VFS: {}", String::from_utf8_lossy(&output.stderr))); }
    }
    Ok(())
}
pub(super) fn view(game: &NexusGame) -> Result<Option<PathBuf>, String> {
    let path = state(game)?.join("merged");
    Ok(mounted(&path).then_some(path))
}
pub(super) fn mount(game: &NexusGame, runtime: &Path, root: &Path) -> Result<PathBuf, String> {
    if !vfs_available() { return Err("VFS requer fuse-overlayfs, fusermount3, bubblewrap e acesso a /dev/fuse.".into()); }
    unmount(game)?;
    let state = state(game)?;
    if runtime.starts_with(root) || state.starts_with(root) {return Err("O perfil VFS deve ficar fora da pasta do jogo.".into());}
    // The writable layer belongs to this revision; never erase it while mounted.
    let revision = game.profile.as_deref().ok_or("Perfil ausente.")?;
    uuid::Uuid::parse_str(revision).map_err(|_| "Perfil inválido.")?;
    let upper = state.join(revision).join("upper");
    let work = state.join(revision).join("work");
    let merged = state.join("merged");
    for path in [&upper, &work, &merged] { fs::create_dir_all(path).map_err(|e| e.to_string())?; }
    for path in [runtime, root, &upper, &work, &merged] {
        if path.to_string_lossy().chars().any(|c| [':', ',', '\\', '\n'].contains(&c)) { return Err("O caminho contém caracteres incompatíveis com o VFS.".into()); }
    }
    let options = format!("lowerdir={}:{},upperdir={},workdir={}",runtime.display(),root.display(),upper.display(),work.display());
    let output = Command::new(helper("fuse-overlayfs").unwrap()).args(["-o", &options]).arg(&merged).output().map_err(|e| e.to_string())?;
    if !output.status.success() || !mounted(&merged) { return Err(format!("Não foi possível montar o VFS: {}",String::from_utf8_lossy(&output.stderr))); }
    Ok(merged)
}
pub(super) fn wrap(command: Command, merged: &Path, root: &Path) -> Result<Command, String> {
    let mut wrapped = Command::new(helper("bwrap").ok_or("bubblewrap ausente.")?);
    wrapped.args(["--bind", "/", "/", "--bind"]).arg(merged).arg(root).arg("--").arg(command.get_program()).args(command.get_args());
    if let Some(cwd) = command.get_current_dir() { wrapped.current_dir(cwd); }
    for (key,value) in command.get_envs() { if let Some(value) = value {wrapped.env(key,value);} else {wrapped.env_remove(key);} }
    wrapped.stdin(Stdio::null());
    Ok(wrapped)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn links_share_content_and_backup_restores_the_link() {
        use std::os::unix::fs::MetadataExt;
        let base = std::env::temp_dir().join(format!("peligames-links-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&base).unwrap();
        let source=base.join("source"); fs::write(&source,b"mod").unwrap();
        let hard=base.join("hard"); Method::Hardlink.place(&source,&hard).unwrap();
        assert_eq!(fs::metadata(&source).unwrap().ino(),fs::metadata(&hard).unwrap().ino());
        let link=base.join("link"); Method::Symlink.place(&source,&link).unwrap();
        let saved=base.join("saved"); backup(&link,&saved).unwrap();
        assert_eq!(fs::read_link(saved).unwrap(),source);
        fs::remove_file(link).unwrap(); assert_eq!(fs::read(&hard).unwrap(),b"mod");
        fs::remove_dir_all(base).unwrap();
    }
}
