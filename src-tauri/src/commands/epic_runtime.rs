//! Experimental Epic integration. The running client keeps owning authentication and launch.
//! Its shared Proton entry point dispatches using immutable per-game prefix/executable metadata.
use super::ExternalGame;
#[path = "../../launch-runtime/src/epic_contract.rs"]
mod native;
use native::{hash, Route, Runtime, MARKER, SENTINEL};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
const LEGACY_DISPATCH_SHA256: &str =
    "4ac3cb8c1b90d2ede89ffed9b0ddc598b8e14126be051eb7cc9242be83783f5d";
fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn canonical(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| match (path.parent(), path.file_name()) {
            (Some(parent), Some(name)) => canonical(parent).join(name),
            _ => path.to_path_buf(),
        })
}
fn private_directory(path: &Path) -> Result<(), String> {
    if path.is_symlink() {
        return Err("A integração Epic não aceita diretórios simbólicos.".into());
    }
    fs::create_dir_all(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn atomic(path: &Path, bytes: &[u8], executable: bool) -> Result<(), String> {
    use std::io::Write;
    if path.is_symlink() {
        return Err("Arquivo da integração Epic não pode ser um link simbólico.".into());
    }
    let tmp = path.with_file_name(format!(".peligames-{}.part", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            file.set_permissions(fs::Permissions::from_mode(if executable {
                0o700
            } else {
                0o600
            }))
            .map_err(|e| e.to_string())?;
        }
        file.write_all(bytes).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        fs::rename(&tmp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result
}
fn route(
    game: &ExternalGame,
    proton: Option<&Path>,
    compat: &Path,
    legacy: &Path,
) -> Result<Route, String> {
    let mut command = crate::core::advanced_settings::configured_launch_command(
        Path::new(SENTINEL),
        Some(&game.advanced),
    )?;
    crate::core::mangohud_config::apply_game_logs(
        &mut command,
        &game.name,
        game.advanced.mangohud_preset.as_ref(),
    )?;
    let custom = super::custom_options(&game.advanced)?;
    crate::core::launch_environment::apply(&mut command, &custom);
    let mut environment: BTreeMap<String, Option<String>> = command
        .get_envs()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    if environment
        .keys()
        .any(|key| matches!(key.as_str(), "WINEPREFIX" | "STEAM_COMPAT_DATA_PATH"))
    {
        return Err("O prefixo dos jogos Epic é gerenciado pela Epic e não pode ser alterado nas variáveis.".into());
    }
    // These belong to the client; each game's environment stays local to this invocation.
    environment.remove("LD_LIBRARY_PATH");
    let mut words = vec![command.get_program().to_string_lossy().into_owned()];
    words.extend(command.get_args().map(|a| a.to_string_lossy().into_owned()));
    Ok(Route {
        id: game.app_id.clone().ok_or("Identificação Epic ausente.")?,
        compat: canonical(compat).to_string_lossy().into_owned(),
        prefix: canonical(&compat.join("pfx"))
            .to_string_lossy()
            .into_owned(),
        executable: game.executable.as_deref().map(|p| {
            Path::new(p)
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(p))
                .to_string_lossy()
                .into_owned()
        }),
        proton: proton.map(|p| p.join("proton").to_string_lossy().into_owned()),
        command: words,
        environment,
        arguments: custom
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect(),
        legacy_adapter: legacy.to_string_lossy().into_owned(),
    })
}
fn install_runtime(directory: &Path, dispatch: &Path) -> Result<(), String> {
    let path = directory.join("proton");
    if directory.is_symlink() || path.is_symlink() {
        return Err("Runtime Epic simbólico não pode ser adaptado.".into());
    }
    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let manifest = directory.join(".peligames-runtime.json");
    let state = if bytes.starts_with(format!("#!/bin/sh\n{MARKER}\n").as_bytes()) {
        let state: Runtime = serde_json::from_str(
            &fs::read_to_string(&manifest)
                .map_err(|_| "O adaptador Epic perdeu seu registro de recuperação.")?,
        )
        .map_err(|e| e.to_string())?;
        if Path::new(&state.backup).components().count() != 1
            || !state.backup.starts_with("proton.peligames-original-")
        {
            return Err("Registro do runtime Epic inválido.".into());
        }
        let backup = directory.join(&state.backup);
        if backup.is_symlink()
            || hash(&fs::read(backup).map_err(|e| e.to_string())?) != state.original_sha256
            || hash(&bytes) != state.shim_sha256
        {
            return Err(
                "A integridade do adaptador Epic ou do Proton original foi alterada.".into(),
            );
        }
        state
    } else {
        // A new official runtime/update gets its own original; never overwrite an older backup.
        let digest = hash(&bytes);
        // Named by date; the manifest keeps its checksum. Older backups keep their names.
        let backup = (1..=99).map(|n| if n == 1 { format!("proton.peligames-original-{}", crate::core::paths::names::stamp()) } else { format!("proton.peligames-original-{}-{n}", crate::core::paths::names::stamp()) })
            .find(|name| fs::symlink_metadata(directory.join(name)).is_err()).ok_or("Não foi possível reservar o backup do Proton Epic.")?;
        let target = directory.join(&backup);
        if target.exists() {
            if target.is_symlink() || hash(&fs::read(&target).map_err(|e| e.to_string())?) != digest
            {
                return Err("Backup do Proton Epic não confere.".into());
            }
        } else {
            atomic(&target, &bytes, true)?;
        }
        Runtime {
            backup,
            original_sha256: digest,
            shim_sha256: String::new(),
        }
    };
    let backup = quote(&directory.join(&state.backup).to_string_lossy());
    let dispatcher = quote(&dispatch.to_string_lossy());
    let routes = quote(&dispatch.with_file_name("routes.json").to_string_lossy());
    let shim=format!("#!/bin/sh\n{MARKER}\nif [ -x {dispatcher} ] && [ -r {routes} ]; then\n  exec {dispatcher} --epic-runtime {routes} -- {backup} \"$@\"\nfi\nexec {backup} \"$@\"\n");
    let state = Runtime {
        shim_sha256: hash(shim.as_bytes()),
        ..state
    };
    if fs::read(&path).map_err(|e| e.to_string())? != bytes {
        return Err("A Epic atualizou o runtime durante a preparação. Tente novamente.".into());
    }
    atomic(
        &manifest,
        &serde_json::to_vec(&state).map_err(|e| e.to_string())?,
        false,
    )?;
    atomic(&path, shim.as_bytes(), true)
}
fn remove_legacy_dispatch(integration: &Path) -> Result<(), String> {
    let path = integration.join("dispatch.py");
    if fs::symlink_metadata(&path).is_ok_and(|meta| meta.is_file() && meta.len() <= 4 * 1024 * 1024)
        && fs::read(&path).is_ok_and(|bytes| hash(&bytes) == LEGACY_DISPATCH_SHA256)
    {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}
pub(super) fn configure(
    home: &Path,
    game: &ExternalGame,
    proton: Option<&Path>,
) -> Result<(), String> {
    if game.native_linux {
        return Ok(());
    }
    let id = game
        .app_id
        .as_deref()
        .ok_or("Identificação Epic ausente.")?;
    let (_, _, artifact) = super::super::epic_library::parts(id)?;
    let root =
        super::super::epic_library::root_for(home, id).ok_or("Instalação Epic indisponível.")?;
    let compat = root.join("compat/compatdata").join(artifact);
    let runtime = root.join("compat/runtime");
    let dirs: Vec<_> = fs::read_dir(&runtime)
        .map_err(|_| {
            "O runtime da Epic ainda não foi preparado. Abra um jogo Windows pela Epic uma vez."
        })?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir() && p.join("proton").is_file())
        .collect();
    if dirs.is_empty() {
        return Err(
            "O runtime da Epic ainda não foi preparado. Abra um jogo Windows pela Epic uma vez."
                .into(),
        );
    }
    let legacy = root
        .join(".peligames-proton")
        .join(format!("{}.sh", hash(id.as_bytes())));
    let prepared = route(game, proton, &compat, &legacy)?;
    let integration = root.join(".peligames-runtime");
    private_directory(&integration)?;
    let dispatch = integration.join("pelirun");
    atomic(&dispatch, super::super::steam_runtime::HELPER, true)?;
    let routes_file = integration.join("routes.json");
    if routes_file.is_symlink() {
        return Err("Mapa de jogos Epic não pode ser um link simbólico.".into());
    }
    let mut routes: BTreeMap<String, Route> = match fs::read(&routes_file) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|_| "Mapa de jogos da integração Epic inválido.")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => return Err(e.to_string()),
    };
    routes.insert(id.into(), prepared);
    // Install dispatchers first. Without a matching route they retain Epic's original execution.
    for dir in dirs {
        install_runtime(&dir, &dispatch)?;
    }
    // Remove only our old argument adapter, preserving every other game setting.
    let settings = root.join("settings/game-settings.json");
    if let Ok(before) = fs::read_to_string(&settings) {
        if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&before) {
            if let Some(line) = json["games"][id]["additionalCommands"].as_str() {
                let after = line.replace(&format!("{} ", quote(&legacy.to_string_lossy())), "");
                if after != line {
                    json["games"][id]["additionalCommands"] = after.into();
                    super::external_proton::save(
                        &settings,
                        &before,
                        &serde_json::to_string_pretty(&json).map_err(|e| e.to_string())?,
                    )?;
                }
            }
        }
    }
    atomic(
        &routes_file,
        &serde_json::to_vec(&routes).map_err(|e| e.to_string())?,
        false,
    )?;
    // Remove only the exact legacy script after every runtime points to Rust.
    remove_legacy_dispatch(&integration)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::test_support::RetryOutput;
    use serde_json::json;
    use std::process::Command;
    struct Fixture {
        home: PathBuf,
        root: PathBuf,
        runtime: PathBuf,
        runner: PathBuf,
        game: ExternalGame,
    }
    fn script(path: &Path, label: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            path,
            format!("#!/bin/sh\nprintf '%s\\n' '{label}' \"${{PG_EPIC_TEST:-missing}}\" \"$@\"\n"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
    }
    impl Fixture {
        fn new() -> Self {
            let home = std::env::temp_dir()
                .join(format!("peligames-epic-dispatch-{}", uuid::Uuid::new_v4()));
            let root = home.join(".local/share/com.epicgames.eglv2");
            let runtime = root.join("compat/runtime/GE-Default");
            script(&runtime.join("proton"), "EPIC");
            let runner = home.join("Proton with ' quote");
            script(&runner.join("proton"), "SELECTED");
            let directory = home.join("Game with spaces");
            fs::create_dir_all(&directory).unwrap();
            let executable = directory.join("Game.exe");
            fs::write(&executable, b"MZ").unwrap();
            fs::write(root.join("installations.jsonl"),json!({"installationId":"test","complete":true,"namespace":"ns","itemId":"item","artifactId":"game","dir":directory}).to_string()).unwrap();
            let mut game = super::super::tests::game("Epic Games", "ns:item:game");
            game.path = directory.to_string_lossy().into_owned();
            game.directory = game.path.clone();
            game.executable = Some(executable.to_string_lossy().into_owned());
            game.prefix = Some(
                root.join("compat/compatdata/game/pfx")
                    .to_string_lossy()
                    .into_owned(),
            );
            game.advanced.launch_options =
                Some("PG_EPIC_TEST='with spaces' %command% '--extra'".into());
            Self {
                home,
                root,
                runtime,
                runner,
                game,
            }
        }
        fn run(&self, artifact: &str) -> String {
            let output = Command::new(self.runtime.join("proton"))
                .args([
                    "waitforexitandrun",
                    self.game.executable.as_deref().unwrap(),
                ])
                .env(
                    "STEAM_COMPAT_DATA_PATH",
                    self.root.join("compat/compatdata").join(artifact),
                )
                .env(
                    "WINEPREFIX",
                    self.root
                        .join("compat/compatdata")
                        .join(artifact)
                        .join("pfx"),
                )
                .env_remove("PG_EPIC_TEST")
                .output_retrying();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.home);
        }
    }
    #[test]
    fn epic_dispatch_reads_each_launch_and_isolates_other_games() {
        let mut f = Fixture::new();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        let selected = f.run("game");
        assert!(selected.starts_with("SELECTED\nwith spaces\nwaitforexitandrun\n"));
        assert!(selected.ends_with("--extra\n"));
        assert!(f.run("other").starts_with("EPIC\nmissing\n"));
        // No client restart or inherited environment is involved: replace the route and launch again.
        f.game.advanced.launch_options = Some("PG_EPIC_TEST=updated".into());
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        assert!(f.run("game").starts_with("SELECTED\nupdated\n"));
        // Choosing Epic's own adapted runtime never recurses into the dispatcher.
        configure(&f.home, &f.game, Some(&f.runtime)).unwrap();
        assert!(f.run("game").starts_with("EPIC\nupdated\n"));
    }
    #[test]
    fn epic_dispatch_preserves_original_and_recovers_missing_config_or_runner() {
        let f = Fixture::new();
        let original = fs::read(f.runtime.join("proton")).unwrap();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        let state: Runtime =
            serde_json::from_slice(&fs::read(f.runtime.join(".peligames-runtime.json")).unwrap())
                .unwrap();
        assert_eq!(fs::read(f.runtime.join(&state.backup)).unwrap(), original);
        fs::remove_file(f.runner.join("proton")).unwrap();
        assert!(f.run("game").starts_with("EPIC\nwith spaces\n"));
        let log = fs::read_to_string(f.root.join(".peligames-runtime/launch.jsonl")).unwrap();
        assert!(log.contains("selected-unavailable-using-epic"));
        assert!(!log.contains("with spaces"));
        fs::remove_file(f.root.join(".peligames-runtime/routes.json")).unwrap();
        assert!(f.run("game").starts_with("EPIC\nmissing\n"));
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        fs::remove_file(f.root.join(".peligames-runtime/pelirun")).unwrap();
        assert!(f.run("game").starts_with("EPIC\nmissing\n"));
    }
    #[test]
    fn epic_dispatch_handles_runtime_updates_and_detects_backup_corruption() {
        let f = Fixture::new();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        script(&f.runtime.join("proton"), "EPIC-UPDATED");
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        assert!(f.run("other").starts_with("EPIC-UPDATED\nmissing\n"));
        let state: Runtime =
            serde_json::from_slice(&fs::read(f.runtime.join(".peligames-runtime.json")).unwrap())
                .unwrap();
        fs::write(f.runtime.join(state.backup), "corrupted").unwrap();
        assert!(configure(&f.home, &f.game, Some(&f.runner))
            .unwrap_err()
            .contains("integridade"));
    }
    #[test]
    fn epic_dispatch_replaces_the_python_shim_without_losing_the_original() {
        let f = Fixture::new();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        let manifest = f.runtime.join(".peligames-runtime.json");
        let mut state: Runtime = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        let original = fs::read(f.runtime.join(&state.backup)).unwrap();
        let legacy = format!("#!/bin/sh\n{MARKER}\nexec /usr/bin/python3 dispatch.py \"$@\"\n");
        fs::write(f.runtime.join("proton"), &legacy).unwrap();
        state.shim_sha256 = hash(legacy.as_bytes());
        fs::write(&manifest, serde_json::to_vec(&state).unwrap()).unwrap();
        let custom = f.root.join(".peligames-runtime/dispatch.py");
        fs::write(&custom, "custom file; preserve it").unwrap();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        let shim = fs::read_to_string(f.runtime.join("proton")).unwrap();
        assert!(shim.contains("--epic-runtime"));
        assert!(!shim.contains("/usr/bin/python3"));
        assert_eq!(fs::read(f.runtime.join(&state.backup)).unwrap(), original);
        assert_eq!(
            fs::read_to_string(custom).unwrap(),
            "custom file; preserve it"
        );
        assert!(f.run("game").starts_with("SELECTED\nwith spaces\n"));
        assert!(f.run("other").starts_with("EPIC\nmissing\n"));
    }
    #[test]
    fn epic_dispatch_protects_prefix_and_cleans_only_legacy_adapter() {
        let mut f = Fixture::new();
        f.game.advanced.launch_options = Some("WINEPREFIX=/different/prefix".into());
        assert!(configure(&f.home, &f.game, Some(&f.runner))
            .unwrap_err()
            .contains("prefixo"));
        assert!(!f.root.join(".peligames-runtime").exists());
        f.game.advanced.launch_options = Some("PG_EPIC_TEST=valid".into());
        fs::create_dir_all(f.root.join("settings")).unwrap();
        let legacy = f
            .root
            .join(".peligames-proton")
            .join(format!("{}.sh", hash(b"ns:item:game")));
        fs::write(f.root.join("settings/game-settings.json"),json!({"games":{"ns:item:game":{"additionalCommandsEnabled":true,"additionalCommands":format!("--keep {} %command%",quote(&legacy.to_string_lossy())),"autoUpdate":false},"other":{"keep":true}},"cloudSaves":{"keep":true}}).to_string()).unwrap();
        configure(&f.home, &f.game, Some(&f.runner)).unwrap();
        let settings: serde_json::Value =
            serde_json::from_slice(&fs::read(f.root.join("settings/game-settings.json")).unwrap())
                .unwrap();
        assert_eq!(
            settings["games"]["ns:item:game"]["additionalCommands"],
            "--keep %command%"
        );
        assert_eq!(settings["games"]["ns:item:game"]["autoUpdate"], false);
        assert_eq!(settings["games"]["other"]["keep"], true);
        assert_eq!(settings["cloudSaves"]["keep"], true);
        let output = Command::new(f.runtime.join("proton"))
            .args([
                "waitforexitandrun",
                f.game.executable.as_deref().unwrap(),
                legacy.to_str().unwrap(),
                "%command%",
                "--kept",
            ])
            .env(
                "STEAM_COMPAT_DATA_PATH",
                f.root.join("compat/compatdata/game"),
            )
            .output_retrying();
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(output.status.success());
        assert!(!text.contains("%command%"));
        assert!(!text.contains(".peligames-proton"));
        assert!(text.contains("--kept"));
    }
}
