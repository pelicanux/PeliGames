//! Pelinstall: the "Open with" entry for Windows installers. It carries no
//! launcher code of its own; it starts the PeliGames binary beside it in the
//! installer module (`peligames --install [FILE]`), which opens only the
//! installation assistant.
use std::{
    path::PathBuf,
    process::{Command, ExitCode},
};

/// The PeliGames binary in the same folder (/opt/PeliGames, AppImage usr/bin, Release).
fn launcher() -> Result<PathBuf, String> {
    let current = std::env::current_exe().map_err(|e| format!("Pelinstall: {e}"))?;
    let current = current.canonicalize().unwrap_or(current);
    let folder = current.parent().ok_or("Pelinstall: pasta do programa indisponível.")?;
    ["peligames", "PeliGames"]
        .iter()
        .map(|name| folder.join(name))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("Pelinstall: o PeliGames não foi encontrado em {}.", folder.display()))
}

/// Same command line as before: no argument asks for a file, one file opens
/// the assistant for it; flags (--launch-game, --verify-ui…) pass unchanged.
fn arguments(args: Vec<String>) -> Vec<String> {
    match args.as_slice() {
        [] => vec!["--install".into()],
        [file] if !file.starts_with('-') => vec!["--install".into(), file.clone()],
        _ => args,
    }
}

fn main() -> ExitCode {
    let launcher = match launcher() {
        Ok(path) => path,
        Err(error) => { eprintln!("{error}"); return ExitCode::FAILURE; }
    };
    let mut command = Command::new(launcher);
    command.args(arguments(std::env::args().skip(1).collect()));
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Replaces this process: the assistant keeps this PID and environment.
        let error = command.exec();
        eprintln!("Pelinstall: {error}");
        ExitCode::FAILURE
    }
    #[cfg(not(unix))]
    match command.status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(_) => ExitCode::FAILURE,
        Err(error) => { eprintln!("Pelinstall: {error}"); ExitCode::FAILURE }
    }
}

#[cfg(test)]
mod tests {
    use super::arguments;
    #[test]
    fn files_open_the_assistant_and_flags_pass_through() {
        let owned = |items: &[&str]| items.iter().map(|item| item.to_string()).collect::<Vec<_>>();
        assert_eq!(arguments(vec![]), owned(&["--install"]));
        assert_eq!(arguments(owned(&["/jogos/setup.exe"])), owned(&["--install", "/jogos/setup.exe"]));
        assert_eq!(arguments(owned(&["--verify-ui"])), owned(&["--verify-ui"]));
        assert_eq!(arguments(owned(&["--launch-game", "/x"])), owned(&["--launch-game", "/x"]));
    }
}
