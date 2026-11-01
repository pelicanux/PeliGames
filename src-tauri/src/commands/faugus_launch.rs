//! Give outer wrappers the same prefix that Faugus will supply later to its runner.
//! No launcher settings or prefix files are changed while building this environment.
use super::ExternalGame;
use std::{ffi::OsStr, path::Path, process::Command};

pub(super) fn apply(game: &ExternalGame, command: &mut Command) -> Result<(), String> {
    if game.launcher != "Faugus" || game.native_linux || game.flatpak {
        return Ok(());
    }
    if !game
        .advanced
        .wrappers
        .iter()
        .any(|w| w.enabled && !w.program.trim().is_empty())
    {
        return Ok(());
    }
    let prefix = game.prefix.as_deref().filter(|s| !s.trim().is_empty()).ok_or(
        "O prefixo deste jogo não foi encontrado no Faugus. Confira a pasta no Faugus e atualize a biblioteca antes de iniciar com wrappers.",
    )?;
    let prefix = Path::new(prefix);
    if !prefix.is_absolute() || !prefix.is_dir() {
        return Err("O prefixo informado pelo Faugus está indisponível. Confira a pasta e atualize a biblioteca.".into());
    }
    let resolved = prefix
        .canonicalize()
        .map_err(|_| "Não foi possível acessar o prefixo do Faugus.")?;
    // Faugus will use its registered prefix even if an outer custom environment
    // asks for another one. Refuse the mismatch before any wrapper writes files.
    for (key, value) in command.get_envs() {
        let Some(value) = value else { continue };
        let candidate = if key == "WINEPREFIX" {
            Path::new(value).to_path_buf()
        } else if key == "STEAM_COMPAT_DATA_PATH" {
            Path::new(value).join("pfx")
        } else {
            continue;
        };
        if !candidate.is_absolute() || candidate.canonicalize().ok().as_ref() != Some(&resolved) {
            return Err("As opções de inicialização indicam outro prefixo. Altere o prefixo no Faugus ou remova WINEPREFIX/STEAM_COMPAT_DATA_PATH das opções deste jogo.".into());
        }
    }
    // The HIP wrapper prioritizes STEAM_COMPAT_DATA_PATH over WINEPREFIX.
    // An inherited Steam context must not redirect its writes to another game.
    command
        .env_remove("STEAM_COMPAT_DATA_PATH")
        .env("WINEPREFIX", OsStr::new(prefix));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{advanced_settings::LaunchWrapper, test_support::RetryOutput};
    use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("peligames-faugus-launch-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(root.join("Prefix with spaces/drive_c/windows/system32")).unwrap();
            Self(root)
        }
        fn game(&self) -> ExternalGame {
            let mut game = super::super::tests::game("Faugus", "test-game");
            game.prefix = Some(
                self.0
                    .join("Prefix with spaces")
                    .to_string_lossy()
                    .into_owned(),
            );
            game.advanced.wrappers.push(LaunchWrapper {
                program: self.0.join("wrapper.sh").to_string_lossy().into_owned(),
                arguments: String::new(),
                enabled: true,
            });
            game
        }
        fn script(&self, name: &str, source: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, source).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn wrapper_receives_prefix_before_faugus_and_preserves_command_and_environment() {
        let f = Fixture::new();
        let game = f.game();
        let wrapper = f.script("wrapper.sh", "#!/bin/sh\nset -eu\ntest -d \"$WINEPREFIX/drive_c/windows/system32\"\ntest -z \"${STEAM_COMPAT_DATA_PATH-}\"\nprintf bridge > \"$WINEPREFIX/drive_c/windows/system32/fixture.txt\"\nexec \"$@\"\n");
        let launcher = f.script(
            "faugus mock.sh",
            "#!/bin/sh\nprintf '%s\\n' \"$WINEPREFIX\" \"$1\" \"$2\" \"$KEEP_SETTING\"\n",
        );
        let mut command = Command::new(wrapper);
        command.args([
            launcher.as_os_str(),
            OsStr::new("--game"),
            OsStr::new("id with spaces;$HOME"),
        ]);
        command.env("KEEP_SETTING", "unchanged");
        apply(&game, &mut command).unwrap();
        let output = command.output_retrying();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = String::from_utf8(output.stdout).unwrap();
        assert_eq!(
            result.lines().collect::<Vec<_>>(),
            vec![
                game.prefix.as_deref().unwrap(),
                "--game",
                "id with spaces;$HOME",
                "unchanged"
            ]
        );
        assert_eq!(
            fs::read_to_string(
                Path::new(game.prefix.as_ref().unwrap())
                    .join("drive_c/windows/system32/fixture.txt")
            )
            .unwrap(),
            "bridge"
        );
        assert!(command
            .get_envs()
            .any(|(key, value)| key == "STEAM_COMPAT_DATA_PATH" && value.is_none()));
    }
    #[test]
    fn rejects_unavailable_or_conflicting_prefixes_before_running_a_wrapper() {
        let f = Fixture::new();
        let mut game = f.game();
        let other = f.0.join("Other prefix");
        fs::create_dir_all(&other).unwrap();
        for key in ["WINEPREFIX", "STEAM_COMPAT_DATA_PATH"] {
            let mut command = Command::new("unused");
            command.env(key, &other);
            assert!(apply(&game, &mut command)
                .unwrap_err()
                .contains("outro prefixo"));
        }
        for prefix in [
            None,
            Some("relative".into()),
            Some(f.0.join("missing").to_string_lossy().into_owned()),
        ] {
            game.prefix = prefix;
            assert!(apply(&game, &mut Command::new("unused")).is_err());
        }
        assert!(fs::read_dir(&other).unwrap().next().is_none());
    }
    #[test]
    fn accepts_matching_custom_prefix_and_symlink_alias() {
        let f = Fixture::new();
        let game = f.game();
        let alias = f.0.join("alias");
        std::os::unix::fs::symlink(game.prefix.as_ref().unwrap(), &alias).unwrap();
        let mut command = Command::new("unused");
        command.env("WINEPREFIX", alias);
        apply(&game, &mut command).unwrap();
        assert!(command.get_envs().any(|(key, value)| key == "WINEPREFIX"
            && value == Some(OsStr::new(game.prefix.as_ref().unwrap()))));
    }
    #[test]
    fn does_not_change_other_launchers_native_flatpak_or_unwrapped_games() {
        let f = Fixture::new();
        let base = f.game();
        let mut cases = Vec::new();
        for launcher in ["Steam", "Epic Games", "Lutris", "Heroic", "Bottles"] {
            let mut game = base.clone();
            game.launcher = launcher.into();
            cases.push(game);
        }
        let mut native = base.clone();
        native.native_linux = true;
        cases.push(native);
        let mut flatpak = base.clone();
        flatpak.flatpak = true;
        cases.push(flatpak);
        let mut disabled = base;
        disabled.advanced.wrappers[0].enabled = false;
        cases.push(disabled);
        for game in cases {
            let mut command = Command::new("unused");
            command
                .env("WINEPREFIX", "/existing")
                .env("STEAM_COMPAT_DATA_PATH", "/existing-compat");
            apply(&game, &mut command).unwrap();
            assert_eq!(
                command
                    .get_envs()
                    .filter(|(_, value)| value.is_some())
                    .count(),
                2
            );
        }
    }
}
