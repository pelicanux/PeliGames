use super::storage::{automount, udisks::Udisks};
use serde_json::{json, Value};
use std::{io::{Read, Write}, process::{Command, Stdio}, sync::Mutex};
static SAVE_LOCK: Mutex<()> = Mutex::new(());
/// Root helper: `peligames --storage-root` through pkexec, request on stdin.
/// One password for the whole change, instead of one polkit prompt per step
/// (folder, unmount, fstab, format, systemd reload, mount).
pub const ROOT_FLAG: &str = "--storage-root";
const ROOT_ACTIONS: [&str; 4] = ["save", "format_ntfs", "prepare_storage", "operation"];
const REQUEST_LIMIT: u64 = 65536;
fn run(request: Value) -> Result<Value, String> {
    let mut system = Udisks::connect()?;
    match request["action"].as_str().unwrap_or("") {
        "list" => {
            let objects = automount::System::objects(&mut system)?;
            let mut listing = automount::storage_listing(&objects);
            for item in &mut listing {
                let shared = item["mounts"].as_array().and_then(|mounts| mounts.first()).and_then(Value::as_str).is_some_and(|mount| is_shared(std::path::Path::new(mount)));
                item["shared_between_systems"] = json!(shared);
            }
            Ok(Value::Array(listing))
        }
        "save" => automount::save(&request, &mut system),
        "operation" => automount::storage_operation(&request, &mut system),
        "format_ntfs" => automount::format_ntfs(&request, &mut system),
        "prepare_storage" => automount::prepare_storage(&request, &mut system),
        _ => Err("Operação inválida.".into()),
    }
}
/// Changes that touch fstab or format run as root after one authorization.
/// An AppImage that is not installed cannot be run by root, so it keeps the
/// per-step polkit prompts.
fn run_admin(request: Value) -> Result<Value, String> {
    let installed = !crate::core::pelinstall::appimage_status().is_ok_and(|status| status.appimage);
    if !installed || !std::path::Path::new("/usr/bin/pkexec").is_file() {
        // UDisks' take-ownership already gave the new filesystem to this user.
        let shared = request["shared_between_systems"] == true;
        let result = run(request)?;
        return if shared { share_mounted(result) } else { Ok(result) };
    }
    let helper = std::env::current_exe().and_then(|path| path.canonicalize()).map_err(|_| "Não foi possível localizar o executável do PeliGames.")?;
    let mut child = Command::new("/usr/bin/pkexec").arg(helper).arg(ROOT_FLAG)
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null())
        .spawn().map_err(|_| "Não foi possível solicitar autorização de administrador.")?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(request.to_string().as_bytes()).map_err(|_| "Não foi possível enviar o pedido ao assistente de administrador.")?;
    }
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let reply: Option<Value> = String::from_utf8_lossy(&output.stdout).lines().last().and_then(|line| serde_json::from_str(line).ok());
    match (output.status.code(), reply) {
        (Some(0), Some(reply)) if reply.get("ok").is_some() => Ok(reply["ok"].clone()),
        (Some(0), Some(reply)) if reply["error"].is_string() => Err(reply["error"].as_str().unwrap_or_default().to_string()),
        (Some(126), _) => Err("A autorização foi cancelada. Nada foi alterado.".into()),
        (Some(127), _) => Err("A autorização de administrador falhou. Verifique o Polkit.".into()),
        _ => Err("O assistente de administrador terminou sem resposta. Atualize a lista para conferir o estado do disco.".into()),
    }
}

/// Root side of `run_admin`. Prints one JSON line: {"ok": result} or {"error": message}.
pub fn run_root() -> i32 {
    let reply = match serve_root() {
        Ok(result) => json!({"ok": result}),
        Err(error) => json!({"error": error}),
    };
    println!("{reply}");
    0
}
fn serve_root() -> Result<Value, String> {
    if unsafe { libc::geteuid() } != 0 {
        return Err("É necessário autorizar como administrador.".into());
    }
    // pkexec sets PKEXEC_UID to the user who asked; new filesystems belong to them.
    let uid: u32 = std::env::var("PKEXEC_UID").ok().and_then(|uid| uid.parse().ok()).ok_or("Usuário solicitante desconhecido.")?;
    let mut input = String::new();
    std::io::stdin().take(REQUEST_LIMIT + 1).read_to_string(&mut input).map_err(|_| "Pedido inválido.")?;
    if input.len() as u64 > REQUEST_LIMIT {
        return Err("Pedido de preparação muito grande.".into());
    }
    let request: Value = serde_json::from_str(&input).map_err(|_| "Pedido inválido.")?;
    if !ROOT_ACTIONS.contains(&request["action"].as_str().unwrap_or("")) || (request["action"] == "operation" && !matches!(request["operation"].as_str(), Some("mount" | "remount"))) {
        return Err("Operação inválida.".into());
    }
    let shared = request["shared_between_systems"] == true;
    let result = run(request)?;
    // As root, UDisks' take-ownership gives the new filesystem to root.
    if result["formatted"] == true {
        if let Some(mount) = result["mount_path"].as_str() {
            give_to(mount, uid).map_err(|error| format!("Formatado e montado em {mount}, mas a pasta ficou sem permissão para o seu usuário: {error}"))?;
        }
    }
    if shared { share_mounted(result) } else { Ok(result) }
}
/// "Use in more than one Linux system": ext4/Btrfs store owners as numbers
/// (UID), and another system's account may have a different one. Everything
/// already on the mounted filesystem becomes writable by everyone, and a
/// default ACL on every folder does the same for what is created later.
fn share_mounted(mut result: Value) -> Result<Value, String> {
    let Some(mount) = result["mount_path"].as_str().map(str::to_string) else { return Ok(result) };
    share_tree(std::path::Path::new(&mount))
        .map_err(|error| format!("O armazenamento está montado em {mount}, mas não foi possível liberar a gravação para outros sistemas: {error}"))?;
    result["shared_between_systems"] = json!(true);
    Ok(result)
}
/// Walks one filesystem without following symlinks or entering other mounts.
/// Folders get 0777 and the default ACL; files keep their execute bits (0777
/// when executable, 0666 otherwise). `lost+found` stays root's.
fn share_tree(root: &std::path::Path) -> Result<(), String> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let device = std::fs::symlink_metadata(root).map_err(|error| error.to_string())?.dev();
    let (mut pending, mut failed, mut first_error) = (vec![root.to_path_buf()], 0usize, None);
    let mut fail = |path: &std::path::Path, error: String| {
        failed += 1;
        first_error.get_or_insert_with(|| format!("{}: {error}", path.display()));
    };
    while let Some(folder) = pending.pop() {
        if let Err(error) = share_folder(&folder) {
            fail(&folder, error);
            continue;
        }
        let Ok(entries) = std::fs::read_dir(&folder) else { fail(&folder, "pasta ilegível".into()); continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else { continue };
            if meta.file_type().is_symlink() || meta.dev() != device || (folder == root && entry.file_name() == "lost+found") {
                continue;
            }
            if meta.is_dir() {
                pending.push(path);
            } else if meta.is_file() {
                let mode = if meta.mode() & 0o111 != 0 { 0o777 } else { 0o666 };
                if meta.mode() & 0o7777 != mode {
                    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)) {
                        fail(&path, error.to_string());
                    }
                }
            }
        }
    }
    match first_error {
        Some(error) => Err(format!("{failed} itens sem permissão alterada (primeiro: {error})")),
        None => Ok(()),
    }
}
/// Whether `path` is a folder shared by `share_folder` (others can write,
/// default ACL gives others rwx). Readable without root.
fn is_shared(path: &std::path::Path) -> bool {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    if !std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o777 == 0o777) {
        return false;
    }
    let Ok(target) = std::ffi::CString::new(path.as_os_str().as_bytes()) else { return false };
    let mut acl = [0u8; 256];
    let size = unsafe { libc::getxattr(target.as_ptr(), c"system.posix_acl_default".as_ptr(), acl.as_mut_ptr().cast(), acl.len()) };
    size > 4 && acl[4..size as usize].chunks_exact(8).any(|entry| entry[..4] == [0x20, 0, 7, 0])
}
fn share_folder(path: &std::path::Path) -> Result<(), String> {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o777)).map_err(|error| error.to_string())?;
    let target = std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| "caminho inválido")?;
    let acl = default_acl();
    let status = unsafe { libc::setxattr(target.as_ptr(), c"system.posix_acl_default".as_ptr(), acl.as_ptr().cast(), acl.len(), 0) };
    if status != 0 {
        return Err(format!("ACL padrão não aceita ({})", std::io::Error::last_os_error()));
    }
    Ok(())
}
/// Kernel POSIX ACL xattr (version 2): owner, group and others with rwx.
/// New files inherit it (masked by the create mode), so umask no longer
/// removes write access for the other systems' accounts.
fn default_acl() -> Vec<u8> {
    const UNDEFINED: u32 = u32::MAX;
    let mut acl = 2u32.to_le_bytes().to_vec();
    for tag in [0x01u16, 0x04, 0x20] { // ACL_USER_OBJ, ACL_GROUP_OBJ, ACL_OTHER
        acl.extend(tag.to_le_bytes());
        acl.extend(7u16.to_le_bytes());
        acl.extend(UNDEFINED.to_le_bytes());
    }
    acl
}
/// The root folder of a freshly formatted, mounted filesystem to `uid` and its group.
fn give_to(mount: &str, uid: u32) -> Result<(), String> {
    let mut buffer = vec![0 as libc::c_char; 16384];
    let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
    let mut found = std::ptr::null_mut();
    let status = unsafe { libc::getpwuid_r(uid, &mut entry, buffer.as_mut_ptr(), buffer.len(), &mut found) };
    if status != 0 || found.is_null() {
        return Err("usuário não encontrado".into());
    }
    std::os::unix::fs::lchown(mount, Some(uid), Some(entry.pw_gid)).map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn list_storage_volumes() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| run(json!({"action":"list"}))).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn list_storage_fstab_entries() -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(|| {
        let text = super::storage::fstab::read()?;
        serde_json::to_value(super::storage::fstab::rows(&text)).map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn remove_storage_fstab_entry(key: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de montagem em andamento.".to_string())?;
        super::storage::fstab::remove(&key)
    }).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn save_storage_automount(id: String, uuid: String, name: String, mount_at_startup: bool, shared_between_systems: Option<bool>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de montagem em andamento.".to_string())?;
        run_admin(json!({"action":"save", "id":id, "uuid":uuid, "name":name, "mount_at_startup":mount_at_startup, "shared_between_systems":shared_between_systems.unwrap_or(false)}))
    }).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn format_ntfs_storage(id: String, uuid: String, name: String, filesystem: String, mount_at_startup: bool, confirmation_device: String, confirmation_word: String, backup_confirmed: bool, shared_between_systems: Option<bool>) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        run_admin(json!({"action":"format_ntfs", "id":id, "uuid":uuid, "name":name,
            "filesystem":filesystem, "mount_at_startup":mount_at_startup,
            "confirmation_device":confirmation_device, "confirmation_word":confirmation_word, "backup_confirmed":backup_confirmed, "shared_between_systems":shared_between_systems.unwrap_or(false)}))
    }).await.map_err(|e| e.to_string())?
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageOperation { Mount, Unmount, RemoveAutomount, Remount }
#[tauri::command]
pub async fn prepare_empty_storage(mut request: Value) -> Result<Value, String> {
    if request.to_string().len() > 65536 { return Err("Pedido de preparação muito grande.".into()); }
    request.as_object_mut().ok_or("Pedido de preparação inválido.")?.insert("action".into(), "prepare_storage".into());
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        run_admin(request)
    }).await.map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn manage_storage_volume(id: String, uuid: String, operation: StorageOperation) -> Result<Value, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let _guard = SAVE_LOCK.try_lock().map_err(|_| "Já existe uma alteração de armazenamento em andamento.".to_string())?;
        // Mounting reloads systemd and mounts a system (fstab) entry: one
        // password as root. Ejecting needs no reload, so it keeps polkit.
        let request = json!({"action":"operation", "id":id, "uuid":uuid, "operation":operation});
        if matches!(request["operation"].as_str(), Some("mount" | "remount")) { run_admin(request) } else { run(request) }
    }).await.map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_acl_matches_kernel_layout() {
        let acl = default_acl();
        assert_eq!(acl.len(), 4 + 3 * 8);
        assert_eq!(&acl[..4], &[2, 0, 0, 0]);
        assert_eq!(&acl[4..12], &[1, 0, 7, 0, 255, 255, 255, 255]);
        assert_eq!(&acl[20..24], &[0x20, 0, 7, 0]);
    }
    /// Real check on a filesystem with ACL support (ext4/Btrfs/tmpfs): files
    /// created later are writable by everyone despite a restrictive umask.
    #[test]
    fn shared_folder_makes_new_files_writable_by_others() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("peligames-share-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        if share_folder(&dir).is_err() {
            std::fs::remove_dir_all(&dir).unwrap();
            return; // no ACL support on this temp filesystem
        }
        let old = unsafe { libc::umask(0o077) };
        std::fs::write(dir.join("save.dat"), b"x").unwrap();
        std::fs::create_dir(dir.join("Game")).unwrap();
        unsafe { libc::umask(old) };
        assert_eq!(std::fs::metadata(dir.join("save.dat")).unwrap().permissions().mode() & 0o777, 0o666);
        assert_eq!(std::fs::metadata(dir.join("Game")).unwrap().permissions().mode() & 0o777, 0o777);
        assert!(is_shared(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }
    /// An existing library: files, executables and nested folders become
    /// writable by everyone; symlinks are not followed.
    #[test]
    fn shared_tree_opens_existing_files() {
        use std::os::unix::fs::PermissionsExt;
        let mode = |path: &std::path::Path| std::fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777;
        let dir = std::env::temp_dir().join(format!("peligames-share-tree-{}", uuid::Uuid::new_v4()));
        let outside = std::env::temp_dir().join(format!("peligames-outside-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("Game/bin")).unwrap();
        std::fs::write(&outside, b"x").unwrap();
        std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(dir.join("Game/save.dat"), b"x").unwrap();
        std::fs::set_permissions(dir.join("Game/save.dat"), std::fs::Permissions::from_mode(0o600)).unwrap();
        std::fs::write(dir.join("Game/bin/game.sh"), b"x").unwrap();
        std::fs::set_permissions(dir.join("Game/bin/game.sh"), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(dir.join("Game/bin"), std::fs::Permissions::from_mode(0o700)).unwrap();
        std::os::unix::fs::symlink(&outside, dir.join("link")).unwrap();
        assert!(!is_shared(&dir));
        if share_tree(&dir).is_ok() {
            assert_eq!(mode(&dir.join("Game/save.dat")), 0o666);
            assert_eq!(mode(&dir.join("Game/bin/game.sh")), 0o777);
            assert_eq!(mode(&dir.join("Game/bin")), 0o777);
            assert!(is_shared(&dir.join("Game/bin")));
        }
        assert_eq!(mode(&outside), 0o600, "symlink targets are never changed");
        std::fs::remove_dir_all(&dir).unwrap();
        std::fs::remove_file(&outside).unwrap();
    }
}
