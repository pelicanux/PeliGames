mod profile;
mod release_features;
mod discord_socket;

mod profile_io;
use profile::app_id;
use std::ffi::OsString;
use std::os::unix::process::CommandExt;
use std::process::Command;

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mode = args.next().and_then(|v| v.into_string().ok()).ok_or("Use pelirun --profile ID -- comando [argumentos].")?;
    let identity = args.next().and_then(|v| v.into_string().ok()).ok_or("Identificador do perfil ausente.")?;
    match mode.as_str() {
        "--profile" => { profile::profile_id(&identity)?; }
        "--app" => { app_id(&identity)?; }
        _ => return Err("Use pelirun --profile ID -- comando [argumentos].".into()),
    }
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--")) {
        return Err("Separador do comando ausente.".into());
    }
    let original: Vec<OsString> = args.collect();
    if original.is_empty() || original[0].is_empty() {
        return Err("O launcher não forneceu o comando do jogo.".into());
    }
    // Survives a launcher reset: if its JSON is removed, launch the original
    // Steam command. Never delete this executable while Steam references it.
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = executable
        .parent()
        .ok_or("Diretório do auxiliar indisponível.")?;
    let mut profile = if mode == "--profile" {
        profile_io::universal(&directory.join("profiles"), &identity)?
    } else {
        profile_io::legacy(&directory.join("profiles").join(format!("{identity}.json")), app_id(&identity)?)?
    };
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
        // Proton's Discord bridge reaches a Flatpak/Snap client only through a top-level socket.
        if profile.environment.get("PROTON_DISCORD_BRIDGE").is_some_and(|value| value.as_deref() == Some("1")) {
            if let Err(error) = discord_socket::link_sandboxed(&discord_socket::runtime_dir()) { eprintln!("PeliGames: {error}"); }
        }
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
