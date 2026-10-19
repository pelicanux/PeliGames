//! Process identity and scoped signalling. Never uses pkill or process groups.
use std::{fs, os::{fd::{AsRawFd, FromRawFd, OwnedFd}, unix::fs::MetadataExt}, path::{Path, PathBuf}};

#[derive(Clone, Debug)]
pub(super) struct Process { pub pid: i32, pub start: u64, pub executable: PathBuf, pub scope: Option<bool> }

fn stat(path: &Path) -> Option<(u8, i32, u64)> {
    let text = fs::read_to_string(path.join("stat")).ok()?;
    let (_, fields) = text.rsplit_once(')')?;
    let fields: Vec<_> = fields.split_whitespace().collect();
    Some((fields.first()?.as_bytes().first().copied()?, fields.get(1)?.parse().ok()?, fields.get(19)?.parse().ok()?))
}

fn scope(path: &Path) -> Option<bool> {
    match fs::read_to_string(path.join("root/.flatpak-info")) {
        Ok(info) => info.lines().any(|line| line.trim() == "name=com.valvesoftware.Steam").then_some(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => match fs::read(path.join("environ")) {
            Ok(env) => Some(env.split(|b| *b == 0).any(|v| v == b"FLATPAK_ID=com.valvesoftware.Steam")),
            Err(_) => None,
        },
        Err(_) => match fs::read(path.join("environ")) {
            Ok(env) => Some(env.split(|b| *b == 0).any(|v| v == b"FLATPAK_ID=com.valvesoftware.Steam")),
            Err(_) => None,
        },
    }
}

pub(super) fn clients(proc: &Path, flatpak: bool) -> Result<Vec<Process>, String> {
    let mut result = Vec::new();
    for entry in fs::read_dir(proc).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<i32>().ok()).filter(|p| *p > 0) else { continue };
        if !entry.metadata().is_ok_and(|m| m.uid() == unsafe { libc::getuid() }) { continue; }
        let args = fs::read(path.join("cmdline")).unwrap_or_default();
        // These are IPC requesters, not the client whose config is being edited.
        if args.split(|b| *b == 0).any(|arg| arg == b"-shutdown") { continue; }
        let executable = fs::read_link(path.join("exe")).unwrap_or_default();
        if !executable.file_name().is_some_and(|v| v == "steam") && !args.split(|b| *b == 0).take(2)
            .filter_map(|arg| std::str::from_utf8(arg).ok()).any(|arg| Path::new(arg).file_name().is_some_and(|v| v == "steam")) { continue; }
        let Some((state, _, start)) = stat(&path) else {
            if path.exists() { return Err("Não foi possível verificar a identidade do processo da Steam. As configurações foram preservadas.".into()); }
            continue;
        };
        if matches!(state, b'Z' | b'X') { continue; }
        let scope = scope(&path);
        if scope.is_none_or(|value| value == flatpak) { result.push(Process {pid,start,executable,scope}); }
    }
    Ok(result)
}

fn game_id(env: &[u8]) -> bool {
    env.split(|b| *b == 0).any(|entry| {
        [b"SteamAppId=".as_slice(), b"SteamGameId=".as_slice()].iter().any(|key| {
            entry.strip_prefix(*key).and_then(|v| std::str::from_utf8(v).ok()).and_then(|v| v.parse::<u64>().ok()).is_some_and(|v| v != 0)
        })
    })
}

pub(super) fn games_running(proc: &Path, flatpak: bool, client_pids: &[i32]) -> Result<bool, String> {
    let mut unreadable = Vec::new();
    let mut parents = std::collections::HashMap::new();
    for entry in fs::read_dir(proc).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if !entry.metadata().is_ok_and(|m| m.uid() == unsafe { libc::getuid() }) { continue; }
        let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<i32>().ok()) else { continue };
        let Some((state, parent, _)) = stat(&path) else { continue };
        if matches!(state,b'Z'|b'X') { continue; }
        parents.insert(pid,parent);
        if scope(&path).is_some_and(|value| value != flatpak) { continue; }
        let executable = fs::read_link(path.join("exe")).unwrap_or_default();
        let comm = fs::read_to_string(path.join("comm")).unwrap_or_default();
        // Sandboxed web helpers can deny exe/environ reads. Their kernel name
        // remains readable; they are not games and must not stall every restart.
        let internal = |name: &str| matches!(name, "steam" | "steamwebhelper" | "gameoverlayui"
            | "steam-runtime-launcher-service" | "steam-runtime-supervisor" | "steam-runtime-l" | "steam-runtime-s");
        if executable.file_name().and_then(|s| s.to_str()).is_some_and(internal) || internal(comm.trim()) { continue; }
        match fs::read(path.join("environ")) {
            Ok(env) if game_id(&env) => return Ok(true),
            Err(_) if path.exists() => unreadable.push(pid),
            _ => {},
        }
    }
    for mut pid in unreadable {
        for _ in 0..128 {
            if client_pids.contains(&pid) { return Err("Não foi possível confirmar se há um jogo da Steam em execução. Feche os jogos antes de tentar novamente.".into()); }
            match parents.get(&pid) { Some(parent) if *parent != pid => pid = *parent, _ => break }
        }
    }
    Ok(false)
}

pub(super) fn signal(original: &Process, flatpak: bool, roots: &[PathBuf], force: bool) -> Result<(), String> {
    if original.scope != Some(flatpak) { return Err("A instalação do processo da Steam não pôde ser confirmada. Feche o cliente manualmente.".into()); }
    let known = roots.iter().filter_map(|p| p.canonicalize().ok()).any(|root| {
        ["ubuntu12_32/steam", "ubuntu12_64/steam", "steam"].iter().any(|leaf| root.join(leaf) == original.executable)
    });
    if !known { return Err("O processo restante não é o cliente principal confirmado da Steam. Feche a Steam manualmente; as alterações continuam pendentes.".into()); }
    let fd = unsafe { libc::syscall(libc::SYS_pidfd_open, original.pid, 0) };
    if fd < 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) { Ok(()) } else { Err(format!("Não foi possível acessar o cliente da Steam com segurança: {error}")) };
    }
    let fd = unsafe { OwnedFd::from_raw_fd(fd as i32) };
    // Opening the pidfd precedes revalidation, so PID reuse cannot redirect a signal.
    let live = clients(Path::new("/proc"), flatpak)?;
    let Some(current) = live.iter().find(|p| p.pid == original.pid) else { return Ok(()) };
    if current.start != original.start || current.executable != original.executable { return Err("O processo da Steam mudou durante o reinício. Tente novamente.".into()); }
    if games_running(Path::new("/proc"),flatpak,&live.iter().map(|p|p.pid).collect::<Vec<_>>())? {
        return Err("Há um jogo da Steam em execução. O cliente não foi finalizado.".into());
    }
    let result = unsafe { libc::syscall(libc::SYS_pidfd_send_signal, fd.as_raw_fd(), if force {libc::SIGKILL} else {libc::SIGTERM}, std::ptr::null::<libc::siginfo_t>(), 0) };
    if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) { return Err(std::io::Error::last_os_error().to_string()); }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn process(root:&Path,pid:i32,state:&str,args:&[u8],sandbox:bool) {
        let path=root.join(pid.to_string());fs::create_dir_all(path.join("root")).unwrap();
        fs::write(path.join("cmdline"),args).unwrap();fs::write(path.join("environ"),b"").unwrap();
        let mut fields=vec!["0";20];fields[0]=state;fields[1]="1";fields[19]="123";
        fs::write(path.join("stat"),format!("{pid} (name with spaces) {}",fields.join(" "))).unwrap();
        if sandbox {fs::write(path.join("root/.flatpak-info"),"[Application]\nname=com.valvesoftware.Steam\n").unwrap();}
    }
    #[test] fn inventory_excludes_shutdown_requesters_zombies_and_runtime_service() {
        let root=std::env::temp_dir().join(format!("steam-inventory-{}",uuid::Uuid::new_v4()));
        process(&root,1,"S",b"/steam/steam\0",false);process(&root,2,"S",b"/steam/steam\0-shutdown\0",false);
        process(&root,3,"Z",b"/steam/steam\0",false);process(&root,4,"S",b"/steam/steam-runtime-launcher-service\0",false);
        process(&root,5,"S",b"/steam/steam\0",true);
        assert_eq!(clients(&root,false).unwrap().iter().map(|p|p.pid).collect::<Vec<_>>(),[1]);
        assert_eq!(clients(&root,true).unwrap().iter().map(|p|p.pid).collect::<Vec<_>>(),[5]);
        fs::remove_dir_all(root).unwrap();
    }
    #[test] fn games_are_detected_by_exact_environment_keys_in_the_selected_installation() {
        assert!(!game_id(b"SteamAppId=0\0OTHER=SteamAppId=42\0"));assert!(game_id(b"SteamGameId=12345678901234567\0"));
        let root=std::env::temp_dir().join(format!("steam-games-{}",uuid::Uuid::new_v4()));
        process(&root,1,"S",b"game\0",true);fs::write(root.join("1/environ"),b"SteamAppId=42\0").unwrap();
        assert!(!games_running(&root,false,&[]).unwrap());assert!(games_running(&root,true,&[]).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test] fn unverified_processes_are_never_signalled() {
        // Rejections happen before pidfd_open/send_signal; no real process is targeted.
        let mut process=Process{pid:std::process::id() as i32,start:0,executable:"/unrelated/steam".into(),scope:None};
        assert!(signal(&process,false,&[],false).is_err());
        process.scope=Some(true);assert!(signal(&process,false,&[],true).is_err());
        process.scope=Some(false);assert!(signal(&process,false,&[],false).is_err());
    }
}
