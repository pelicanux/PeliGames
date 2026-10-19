mod profile;
mod release_features;

use profile::{app_id, Profile, MAX_BYTES};
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::os::unix::{
    fs::{MetadataExt, OpenOptionsExt},
    process::CommandExt,
};
use std::path::Path;
use std::process::Command;

fn load(path: &Path, app: u32) -> Result<Option<Profile>, String> {
    let file = match fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Não foi possível abrir o perfil: {error}")),
    };
    let meta = file.metadata().map_err(|error| error.to_string())?;
    if !meta.is_file()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o022 != 0
        || meta.len() > MAX_BYTES
    {
        return Err("O perfil de inicialização não é um arquivo privado válido.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("Perfil de inicialização muito grande.".into());
    }
    let profile: Profile = serde_json::from_slice(&bytes)
        .map_err(|error| format!("Perfil de inicialização inválido: {error}"))?;
    profile.validate(app)?;
    Ok(Some(profile))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--app")) {
        return Err("Uso: pelirun --app APPID -- comando-do-jogo [argumentos]".into());
    }
    let app = app_id(
        &args
            .next()
            .and_then(|value| value.into_string().ok())
            .ok_or("Steam AppID ausente.")?,
    )?;
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err("Separador do comando ausente.".into());
    }
    let original: Vec<OsString> = args.collect();
    if original.is_empty() || original[0].is_empty() {
        return Err("A Steam não forneceu o comando do jogo.".into());
    }
    // Survives a launcher reset: if its JSON is removed, launch the original
    // Steam command. Never delete this executable while Steam references it.
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable
        .parent()
        .ok_or("Diretório do auxiliar indisponível.")?;
    let mut profile = load(&directory.join("profiles").join(format!("{app}.json")), app)?;
    if let Some(value) = profile.as_mut() {
        release_features::restrict(&mut value.command, &mut value.environment, release_features::enabled());
    }
    let words = match &profile {
        Some(value) => value.expand(&original)?,
        None => original,
    };
    let mut command = Command::new(&words[0]);
    command.args(&words[1..]);
    if let Some(profile) = profile {
        for (key, value) in profile.environment {
            match value {
                Some(value) => {
                    command.env(key, value);
                }
                None => {
                    command.env_remove(key);
                }
            }
        }
    }
    // Keep the PID, signals, inherited Steam environment and exit status. No
    // shell, background daemon, GUI process, or game-file replacement is used.
    Err(format!(
        "Não foi possível iniciar o jogo: {}",
        command.exec()
    ))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("PeliGames: {error}");
        std::process::exit(1);
    }
}
