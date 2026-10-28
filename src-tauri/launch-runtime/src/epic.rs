//! Epic's native per-invocation bridge. Never records arguments or environment values.
use crate::epic_contract::{hash, Route, Runtime, MARKER, SENTINEL};
const MAX_BYTES: u64 = 4 * 1024 * 1024;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::Command,
};

fn document(path: &Path) -> Result<Vec<u8>, String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|e| e.to_string())?;
    let meta = file.metadata().map_err(|e| e.to_string())?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
        || meta.len() > MAX_BYTES
    {
        return Err("Arquivo da integração Epic inválido ou sem acesso exclusivo.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Arquivo da integração Epic muito grande.".into());
    }
    Ok(bytes)
}
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) if parent != path && !parent.as_os_str().is_empty() => {
                canonical(parent).join(name)
            }
            _ => path.to_path_buf(),
        })
}
fn canonical_env(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(|value| canonical(Path::new(&value)))
}
fn original_runner(path: &Path) -> Result<PathBuf, String> {
    let mut candidate = path.to_path_buf();
    let mut header = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(1024)
        .read_to_end(&mut header)
        .map_err(|e| e.to_string())?;
    if header
        .windows(MARKER.len())
        .any(|word| word == MARKER.as_bytes())
    {
        let state: Runtime =
            serde_json::from_slice(&document(&path.with_file_name(".peligames-runtime.json"))?)
                .map_err(|e| e.to_string())?;
        if Path::new(&state.backup).components().count() != 1
            || !state.backup.starts_with("proton.peligames-original-")
        {
            return Err("Registro do runtime Epic inválido.".into());
        }
        candidate = path.with_file_name(&state.backup);
        if hash(&document(&candidate)?) != state.original_sha256 {
            return Err("A integridade do Proton original foi alterada.".into());
        }
    }
    let path =
        std::ffi::CString::new(candidate.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
    if !candidate.is_file() || unsafe { libc::access(path.as_ptr(), libc::X_OK) } != 0 {
        return Err("Proton selecionado indisponível.".into());
    }
    Ok(candidate)
}
fn select_route(routes: &Path, arguments: &[OsString]) -> Option<Route> {
    let routes: BTreeMap<String, Route> = serde_json::from_slice(&document(routes).ok()?).ok()?;
    let compat = canonical_env("STEAM_COMPAT_DATA_PATH");
    let prefix = canonical_env("WINEPREFIX");
    let executable = arguments.get(1).map(|arg| canonical(Path::new(arg)));
    let mut matches = routes.into_values().filter(|route| {
        (compat
            .as_deref()
            .is_some_and(|path| path == Path::new(&route.compat))
            || prefix
                .as_deref()
                .is_some_and(|path| path == Path::new(&route.prefix)))
            && route
                .executable
                .as_deref()
                .is_none_or(|path| executable.as_deref() == Some(Path::new(path)))
    });
    let route = matches.next()?;
    if matches.next().is_some()
        || route.command.first().is_none_or(|word| word.is_empty())
        || route
            .command
            .iter()
            .filter(|word| word.as_str() == SENTINEL)
            .count()
            != 1
        || route
            .command
            .iter()
            .chain(&route.arguments)
            .any(|word| word.contains('\0'))
        || route.environment.iter().any(|(key, value)| {
            key.is_empty()
                || key.contains(['=', '\0'])
                || value.as_ref().is_some_and(|value| value.contains('\0'))
        })
    {
        return None;
    }
    Some(route)
}
fn record(directory: &Path, game: &str, runner: &Path, status: &str) {
    // Logging must never prevent a game from launching.
    let _ = (|| -> std::io::Result<()> {
        let path = directory.join("launch.jsonl");
        let mut options = OpenOptions::new();
        options
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let mut file = options.open(&path)?;
        if !file.metadata()?.is_file() {
            return Ok(());
        }
        if file.metadata()?.len() > 1024 * 1024 {
            fs::rename(&path, directory.join("launch.previous.jsonl"))?;
            file = options.open(&path)?;
        }
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        writeln!(
            file,
            "{}",
            serde_json::json!({"time":time,"pid":std::process::id(),"game":game,"runner":runner,"status":status})
        )
    })();
}
fn launch(routes: &Path, original: &OsStr, arguments: Vec<OsString>) -> Command {
    let Some(mut route) = select_route(routes, &arguments) else {
        let mut command = Command::new(original);
        command.args(arguments);
        return command;
    };
    let mut runner = PathBuf::from(original);
    let mut status = "epic-default";
    if let Some(selected) = &route.proton {
        match original_runner(Path::new(selected)) {
            Ok(selected) => {
                runner = selected;
                status = "selected";
            }
            Err(_) => status = "selected-unavailable-using-epic",
        }
    }
    crate::release_features::restrict(
        &mut route.command,
        &mut route.environment,
        crate::release_features::enabled(),
    );
    let mut words: Vec<OsString> = route
        .command
        .iter()
        .map(|word| {
            if word == SENTINEL {
                runner.as_os_str().to_owned()
            } else {
                OsString::from(word)
            }
        })
        .collect();
    let mut args = arguments.into_iter().peekable();
    while let Some(arg) = args.next() {
        if arg == OsStr::new(&route.legacy_adapter) {
            if args
                .peek()
                .is_some_and(|word| word == OsStr::new("%command%"))
            {
                args.next();
            }
        } else {
            words.push(arg);
        }
    }
    words.extend(route.arguments.iter().map(OsString::from));
    let mut command = Command::new(&words[0]);
    command.args(&words[1..]);
    for (key, value) in route.environment {
        if matches!(key.as_str(), "WINEPREFIX" | "STEAM_COMPAT_DATA_PATH") {
            continue;
        }
        match value {
            Some(value) => {
                command.env(key, value);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
    if command
        .get_envs()
        .any(|(key, value)| key == "PROTON_DISCORD_BRIDGE" && value == Some(OsStr::new("1")))
    {
        let _ = crate::discord_socket::link_sandboxed(&crate::discord_socket::runtime_dir());
    }
    if let Some(directory) = routes.parent() {
        record(directory, &route.id, &runner, status);
    }
    command
}
pub fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let routes = PathBuf::from(args.next().ok_or("Mapa de jogos Epic ausente.")?);
    if args.next().as_deref() != Some(OsStr::new("--")) {
        return Err("Separador do comando Epic ausente.".into());
    }
    let original = args
        .next()
        .filter(|arg| !arg.is_empty())
        .ok_or("Proton original ausente.")?;
    let mut command = launch(&routes, &original, args.collect());
    Err(format!(
        "Não foi possível iniciar o jogo Epic: {}",
        command.exec()
    ))
}
