//! Editable per-game options; arguments are passed directly, never through a shell.
use super::nexus_local::{NexusGame, Workspace};
use std::{collections::BTreeMap, path::Path, process::Command};

#[derive(Default)]
pub(super) struct Options {
    pub(super) args: Vec<String>,
    pub(super) env: BTreeMap<String, String>,
}
pub(super) fn parse(value: &str) -> Result<Options, String> {
    let normalized = value.replace(['\n', '\r', '\t'], " ");
    let tokens = crate::core::advanced_settings::wrapper_args(&normalized)
        .map_err(|_| "Opções inválidas. Use até 4096 caracteres e confira as aspas.".to_string())?;
    let mut result = Options::default();
    let mut arguments = false;
    for token in tokens {
        if token == "%command%" && !arguments {
            arguments = true;
            continue;
        }
        if !arguments {
            if let Some((key, value)) = token.split_once('=') {
                if !key.is_empty()
                    && key.bytes().enumerate().all(|(i, c)| {
                        c == b'_' || c.is_ascii_alphabetic() || i > 0 && c.is_ascii_digit()
                    })
                {
                    result.env.insert(key.into(), value.into());
                    continue;
                }
            }
        }
        arguments = true;
        result.args.push(token);
    }
    Ok(result)
}
/// Shared syntax for editable launcher arguments and environment variables.
pub(crate) fn validate_custom(value: &str) -> Result<(), String> { parse(value).map(|_| ()) }
pub(crate) fn apply_custom(command: &mut Command, value: &str) -> Result<(), String> {
    apply_options(command, parse(value)?, true);
    Ok(())
}
fn automatic(game: &NexusGame, visible: &Path) -> Options {
    let mut options = Options::default();
    if let Some(module) = super::nexus_deployment::definition(game) {
        options.args = module.launch_arguments.clone();
        let dlls = module
            .dll_overrides
            .iter()
            .filter(|item| {
                !(module.domain == "finalfantasy12"
                    && item.name == "dxgi"
                    && visible.join("x64/dinput8.dll").is_file())
                    && visible.join(&item.file).is_file()
            })
            .map(|item| format!("{}=n,b", item.name))
            .collect::<Vec<_>>();
        if !dlls.is_empty() {
            options
                .env
                .insert("WINEDLLOVERRIDES".into(), dlls.join(";"));
        }
    } else if game.platform == "native" && game.adapter == "valheim" {
        options.args = vec!["-logFile".into(), "-".into()];
    }
    options
}
fn quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}
pub(super) fn describe(options: &Options) -> String {
    options
        .env
        .iter()
        .map(|(key, value)| format!("{key}={}", quote(value)))
        .chain((!options.args.is_empty()).then(|| "%command%".to_string()))
        .chain(options.args.iter().map(|arg| quote(arg)))
        .collect::<Vec<_>>()
        .join(" ")
}
pub(super) fn without_saved_dll_overrides(value: &str) -> Result<Option<String>, String> {
    let mut options = parse(value)?;
    Ok(options.env.remove("WINEDLLOVERRIDES").map(|_| describe(&options)))
}
pub(super) fn apply(game: &NexusGame, command: &mut Command, visible: &Path) -> Result<(), String> {
    let defaults = automatic(game, visible);
    let mut options = match &game.launch_options {
        Some(value) => parse(value)?,
        None => automatic(game, visible),
    };
    // Editing arguments must not remove the DLLs required by installed loaders.
    if let Some(dlls) = defaults.env.get("WINEDLLOVERRIDES") {
        options
            .env
            .entry("WINEDLLOVERRIDES".into())
            .or_insert_with(|| dlls.clone());
    }
    apply_options(command, options, game.launch_options.is_some());
    Ok(())
}
pub(super) fn apply_tool(game: &NexusGame, command: &mut Command, visible: &Path) {
    let mut options = automatic(game, visible);
    options.args.clear();
    apply_options(command, options, false);
}
fn apply_options(command: &mut Command, options: Options, customized: bool) {
    command.args(options.args);
    for (key, value) in options.env {
        // Preserve inherited overrides for automatic mode; an edited value replaces them.
        if key == "WINEDLLOVERRIDES" && !customized {
            let current = std::env::var(&key).unwrap_or_default();
            command.env(
                key,
                if current.is_empty() {
                    value
                } else {
                    format!("{current};{value}")
                },
            );
        } else {
            command.env(key, value);
        }
    }
}
#[derive(serde::Serialize)]
pub struct LaunchOptions {
    automatic: String,
    effective: String,
    customized: bool,
    dll_overrides: String,
}
#[tauri::command]
pub async fn get_nexus_launch_options(game_id: String) -> Result<LaunchOptions, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let game = super::nexus_local::diagnostic_game(&game_id)?;
        let root = Path::new(game.game["directory"].as_str().unwrap_or(""));
        let profile = super::nexus_profile::profile(&game).ok();
        let visible = if game.deploy_method == super::nexus_deploy_methods::Method::Vfs {
            profile.as_deref().unwrap_or(root)
        } else {
            root
        };
        let mut defaults = automatic(&game, visible);
        let detected_dlls = defaults.env.remove("WINEDLLOVERRIDES").unwrap_or_default();
        let automatic = describe(&defaults);
        let mut effective = match &game.launch_options {
            Some(value) => parse(value)?,
            None => defaults,
        };
        let dll_overrides = effective
            .env
            .remove("WINEDLLOVERRIDES")
            .unwrap_or(detected_dlls);
        Ok(LaunchOptions {
            effective: describe(&effective),
            automatic,
            customized: game.launch_options.is_some(),
            dll_overrides,
        })
    })
    .await
    .map_err(|error| error.to_string())?
}
#[tauri::command]
pub async fn configure_nexus_launch_options(
    game_id: String,
    options: Option<String>,
) -> Result<Workspace, String> {
    if let Some(value) = &options {
        if parse(value)?.env.contains_key("WINEDLLOVERRIDES") {
            return Err("Edite WINEDLLOVERRIDES em Prioridade das DLLs do Wine.".into());
        }
    }
    tauri::async_runtime::spawn_blocking(move || {
        // Preserve overrides saved by the previous editor until automatic mode is restored.
        let options = if let Some(value) = options {
            let game = super::nexus_local::diagnostic_game(&game_id)?;
            let mut next = parse(&value)?;
            if let Some(previous) = game.launch_options.as_deref() {
                if let Some(dlls) = parse(previous)?.env.remove("WINEDLLOVERRIDES") {
                    next.env.insert("WINEDLLOVERRIDES".into(), dlls);
                }
            }
            Some(describe(&next))
        } else {
            None
        };
        super::nexus_local::save_launch_options(&game_id, options)
    })
    .await
    .map_err(|error| error.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn main_library_launch_options_persist_and_apply_without_shell() {
        let mut settings = crate::core::advanced_settings::AdvancedSettings::default();
        settings.launch_options = Some("MANGOHUD=1 %command% -windowed \"two words\"".into());
        settings.validate().unwrap();
        let saved = serde_json::to_vec(&settings).unwrap();
        let loaded: crate::core::advanced_settings::AdvancedSettings = serde_json::from_slice(&saved).unwrap();
        assert_eq!(loaded.launch_options, settings.launch_options);
        let mut command = Command::new("/bin/echo");
        command.arg("game.exe");
        apply_custom(&mut command, loaded.launch_options.as_deref().unwrap()).unwrap();
        assert_eq!(command.get_args().map(|v| v.to_string_lossy().into_owned()).collect::<Vec<_>>(), ["game.exe", "-windowed", "two words"]);
        assert!(command.get_envs().any(|(key, value)| key == "MANGOHUD" && value == Some(std::ffi::OsStr::new("1"))));
        settings.launch_options = Some("--name \"unclosed".into());
        assert!(settings.validate().is_err());
        settings.launch_options = None;
        settings.validate().unwrap();
    }
    #[test]
    fn preserves_quoted_arguments_and_environment_without_shell() {
        let options = parse("WINEDLLOVERRIDES=\"winhttp=n,b;version=n,b\" %command% --name \"two words\" \"$(touch /tmp/no)\"").unwrap();
        assert_eq!(options.env["WINEDLLOVERRIDES"], "winhttp=n,b;version=n,b");
        assert_eq!(options.args, ["--name", "two words", "$(touch /tmp/no)"]);
        assert_eq!(parse(&describe(&options)).unwrap().args, options.args);
        assert!(parse("--name \"unclosed").is_err());
        let equals = parse("%command% foo=bar \"C:\\\\mods\\\\file\"").unwrap();
        let roundtrip = parse(&describe(&equals)).unwrap();
        assert!(roundtrip.env.is_empty());
        assert_eq!(roundtrip.args, equals.args);
        assert_eq!(parse("-a\n-b").unwrap().args, ["-a", "-b"]);
        assert!(parse(&"x".repeat(4097)).is_err());
    }
}

#[tauri::command]
pub async fn configure_nexus_advanced(
    game_id: String,
    settings: crate::core::advanced_settings::AdvancedSettings,
) -> Result<Workspace, String> {
    settings.validate()?;
    tauri::async_runtime::spawn_blocking(move || {
        super::nexus_local::save_advanced(&game_id, settings)
    })
    .await
    .map_err(|e| e.to_string())?
}
/// Reuse the launcher wrappers and environment settings while preserving mod loader defaults.
pub(super) fn advanced_command(game: &NexusGame, original: Command) -> Result<Command, String> {
    if game.platform == "native" { game.advanced.validate_native()?; }
    if game.platform == "proton" {
        game.advanced.validate_runner(Path::new(&game.proton))?;
        if game.advanced.options.get("steam_client") == Some(&true) && !super::external_library::launcher_open("Steam") {
            return Err("Abra a Steam e entre na sua conta antes de iniciar um jogo com a comunicação Steam ativada.".into());
        }
    }
    let mut command = crate::core::advanced_settings::launch_command(
        Path::new(original.get_program()),
        Some(&game.advanced),
    )?;
    let advanced_env: Vec<_> = command
        .get_envs()
        .filter_map(|(k, v)| v.map(|v| (k.to_os_string(), v.to_os_string())))
        .collect();
    command.args(original.get_args());
    if let Some(dir) = original.get_current_dir() {
        command.current_dir(dir);
    }
    for (key, value) in original.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    for (key, value) in advanced_env {
        if key == "WINEDLLOVERRIDES" {
            let base = original
                .get_envs()
                .find(|(k, _)| *k == "WINEDLLOVERRIDES")
                .and_then(|(_, v)| v)
                .map(|v| v.to_string_lossy().into_owned())
                .unwrap_or_default();
            command.env(
                key,
                if base.is_empty() {
                    value
                } else {
                    merge_dll_overrides(&base, &value.to_string_lossy()).into()
                },
            );
        } else {
            command.env(key, value);
        }
    }
    if game.platform == "proton" && game.advanced.options.get("steam_client") == Some(&true) {
        // Direct Proton invocations already receive Steam's directory. UMU,
        // however, replaces it: restore it at the compatibility-tool boundary.
        if original.get_envs().any(|(key, _)| key == "PROTONPATH") {
            command.env("PROTONPATH", crate::core::steam_client::runner(Path::new(&game.proton))?);
        }
    }
    crate::core::mangohud_config::apply_with_preset(&mut command, game.advanced.options.get("mangohud") == Some(&true) || game.advanced.options.get("mangohud_env") == Some(&true), game.advanced.mangohud_fps_limit, game.advanced.mangohud_enabled, game.advanced.mangohud_preset.as_ref())?;
    crate::core::mangohud_config::apply_game_logs(&mut command, game.game["name"].as_str().unwrap_or("Jogo"), game.advanced.mangohud_preset.as_ref())?;
    Ok(command)
}

#[cfg(test)]
mod advanced_tests {
    use super::*;
    #[test]
    fn custom_dll_priorities_replace_matching_names_and_keep_other_loaders() {
        assert_eq!(
            merge_dll_overrides("WinHTTP=n,b;version=n,b", "winhttp=b;dinput8=n,b"),
            "dinput8=n,b;version=n,b;winhttp=b"
        );
        assert_eq!(
            merge_dll_overrides("", "dxgi,version=n,b"),
            "dxgi=n,b;version=n,b"
        );
    }
    #[test]
    fn wrappers_preserve_game_arguments_directory_and_loader_environment() {
        let mut game: NexusGame = serde_json::from_value(
            serde_json::json!({"id":"fixture","game":{},"platform":"native","mods":[]}),
        )
        .unwrap();
        game.advanced
            .wrappers
            .push(crate::core::advanced_settings::LaunchWrapper {
                program: "/bin/echo".into(),
                arguments: "--wrapper \"two words\"".into(),
                enabled: true,
            });
        game.advanced.options.insert("dll_overrides".into(), true);
        game.advanced.dll_overrides = "version=n,b".into();
        let mut original = Command::new("/game/executable");
        original
            .args(["--name", "two words"])
            .current_dir("/game")
            .env("SteamAppId", "123")
            .env("WINEDLLOVERRIDES", "winhttp=n,b");
        let result = advanced_command(&game, original).unwrap();
        assert_eq!(result.get_program(), "/bin/echo");
        assert_eq!(
            result.get_args().collect::<Vec<_>>(),
            [
                "--wrapper",
                "two words",
                "/game/executable",
                "--name",
                "two words"
            ]
        );
        assert_eq!(result.get_current_dir(), Some(Path::new("/game")));
        let env: BTreeMap<_, _> = result.get_envs().collect();
        assert_eq!(
            env[std::ffi::OsStr::new("SteamAppId")],
            Some(std::ffi::OsStr::new("123"))
        );
        assert_eq!(
            env[std::ffi::OsStr::new("WINEDLLOVERRIDES")],
            Some(std::ffi::OsStr::new("version=n,b;winhttp=n,b"))
        );
    }
}

fn merge_dll_overrides(base: &str, custom: &str) -> String {
    let mut dlls = BTreeMap::new();
    for entry in base.split(';').chain(custom.split(';')) {
        if let Some((names, order)) = entry.split_once('=') {
            for name in names.split(',').filter(|name| !name.trim().is_empty()) {
                dlls.insert(name.trim().to_ascii_lowercase(), order.trim());
            }
        }
    }
    dlls.into_iter()
        .map(|(name, order)| format!("{name}={order}"))
        .collect::<Vec<_>>()
        .join(";")
}
