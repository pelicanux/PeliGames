//! Rewrite only Steam's Proton/runtime layer; preserve the caller and game arguments.
use crate::profile::steam_proton::SteamProton;
use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

pub struct Launch {
    pub command: Vec<OsString>,
    pub environment: BTreeMap<String, OsString>,
}
fn executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
fn same(first: &Path, second: &Path) -> bool {
    first == second
        || first
            .canonicalize()
            .ok()
            .zip(second.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}
fn paths(
    value: Option<&OsString>,
    remove: &[PathBuf],
    append: &[PathBuf],
) -> Result<OsString, String> {
    let mut result: Vec<PathBuf> = value
        .into_iter()
        .flat_map(std::env::split_paths)
        .filter(|p| !remove.iter().any(|old| same(p, old)))
        .collect();
    for path in append {
        if !result.iter().any(|p| same(p, path)) {
            result.push(path.clone());
        }
    }
    std::env::join_paths(result)
        .map_err(|_| "O caminho do Proton não pode conter dois-pontos.".into())
}
impl SteamProton {
    pub fn launch(
        &self,
        original: &[OsString],
        inherited: &BTreeMap<String, OsString>,
    ) -> Result<Launch, String> {
        self.validate()?;
        for name in ["SteamAppId", "SteamGameId"] {
            if inherited
                .get(name)
                .and_then(|v| v.to_str())
                .is_some_and(|id| id != "0" && id != self.app_id.to_string())
            {
                return Err("O Proton do perfil pertence a outro jogo Steam.".into());
            }
        }
        let game = original
            .iter()
            .position(|p| {
                Path::new(p)
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
            })
            .unwrap_or(original.len());
        let candidates: Vec<_> = original
            .windows(2)
            .enumerate()
            .filter(|(i, p)| {
                *i < game
                    && Path::new(&p[0]).is_absolute()
                    && Path::new(&p[0]).file_name() == Some(OsStr::new("proton"))
                    && matches!(
                        p[1].to_str(),
                        Some("run" | "waitforexitandrun" | "runinprefix")
                    )
            })
            .map(|(i, _)| i)
            .collect();
        if candidates.len() != 1 {
            return Err("A Steam não forneceu um comando Proton reconhecido. Ative a compatibilidade Proton na Steam para este jogo.".into());
        }
        let index = candidates[0];
        let old_proton = Path::new(&original[index])
            .parent()
            .ok_or("Comando Proton inválido.")?
            .to_path_buf();
        let selected=Path::new(&self.directory).canonicalize().map_err(|_|"O Proton escolhido no PeliGames não está disponível. Escolha outra versão ou volte a usar o Proton da Steam.")?;
        if !executable(&selected.join("proton")) {
            return Err("O executável do Proton escolhido não está disponível.".into());
        }
        let runtime = self
            .runtime
            .as_ref()
            .map(|r| {
                Path::new(r)
                    .canonicalize()
                    .map_err(|_| "O Steam Linux Runtime exigido pelo Proton não está disponível.")
            })
            .transpose()?;
        if runtime
            .as_ref()
            .is_some_and(|r| !executable(&r.join("_v2-entry-point")))
        {
            return Err("O Steam Linux Runtime escolhido está incompleto.".into());
        }
        let layers: Vec<_> = original[..index]
            .iter()
            .enumerate()
            .filter(|(_, p)| Path::new(p).file_name() == Some(OsStr::new("_v2-entry-point")))
            .map(|(i, _)| i)
            .collect();
        if layers.len() > 1 {
            return Err("Comando Steam com múltiplos runtimes não suportado.".into());
        }
        let layer = layers.first().copied();
        if layer.is_some_and(|i| {
            i + 3 != index
                || !original[i + 1].to_string_lossy().starts_with("--verb=")
                || original[i + 2] != "--"
        }) {
            return Err("Formato do Steam Linux Runtime não reconhecido; o comando original foi preservado.".into());
        }
        let old_runtime = layer
            .and_then(|i| Path::new(&original[i]).parent())
            .map(Path::to_path_buf);
        let start = layer.unwrap_or(index);
        let mut command = original[..start].to_vec();
        if let Some(path) = &runtime {
            command.extend([
                path.join("_v2-entry-point").into_os_string(),
                format!("--verb={}", original[index + 1].to_string_lossy()).into(),
                "--".into(),
            ]);
        }
        command.push(selected.join("proton").into_os_string());
        command.extend_from_slice(&original[index + 1..]);
        let mut remove = vec![old_proton];
        remove.extend(old_runtime);
        let mut append = vec![selected];
        append.extend(runtime);
        let environment = BTreeMap::from([
            (
                "STEAM_COMPAT_TOOL_PATHS".into(),
                paths(inherited.get("STEAM_COMPAT_TOOL_PATHS"), &remove, &append)?,
            ),
            (
                "STEAM_COMPAT_MOUNTS".into(),
                paths(inherited.get("STEAM_COMPAT_MOUNTS"), &[], &append)?,
            ),
        ]);
        Ok(Launch {
            command,
            environment,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "peli-proton-profile-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            for dir in ["old", "new runner", "old-runtime", "new-runtime"] {
                fs::create_dir_all(root.join(dir)).unwrap();
                for name in ["proton", "_v2-entry-point"] {
                    let p = root.join(dir).join(name);
                    fs::write(&p, "#!/bin/sh\n").unwrap();
                    fs::set_permissions(p, fs::Permissions::from_mode(0o700)).unwrap();
                }
            }
            Self(root)
        }
        fn choice(&self) -> SteamProton {
            SteamProton {
                app_id: 42,
                directory: self.0.join("new runner").to_string_lossy().into_owned(),
                runtime: Some(self.0.join("new-runtime").to_string_lossy().into_owned()),
            }
        }
        fn command(&self) -> Vec<OsString> {
            vec![
                "/steam/reaper".into(),
                "SteamLaunch".into(),
                "AppId=42".into(),
                "--".into(),
                self.0.join("old-runtime/_v2-entry-point").into(),
                "--verb=waitforexitandrun".into(),
                "--".into(),
                self.0.join("old/proton").into(),
                "waitforexitandrun".into(),
                "/game/Game.exe".into(),
                "two words".into(),
                "$(literal)".into(),
            ]
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn replaces_runner_runtime_and_mounts_preserving_the_full_steam_command() {
        let f = Fixture::new();
        let before = f.command();
        let env = BTreeMap::from([
            ("SteamAppId".into(), "42".into()),
            (
                "STEAM_COMPAT_TOOL_PATHS".into(),
                std::env::join_paths([
                    f.0.join("old"),
                    f.0.join("old-runtime"),
                    PathBuf::from("/unrelated"),
                ])
                .unwrap(),
            ),
        ]);
        let launch = f.choice().launch(&before, &env).unwrap();
        assert_eq!(&launch.command[..4], &before[..4]);
        assert_eq!(&launch.command[8..], &before[8..]);
        assert_eq!(launch.command[4], f.0.join("new-runtime/_v2-entry-point"));
        assert_eq!(launch.command[7], f.0.join("new runner/proton"));
        let paths: Vec<_> =
            std::env::split_paths(&launch.environment["STEAM_COMPAT_TOOL_PATHS"]).collect();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("/unrelated"),
                f.0.join("new runner"),
                f.0.join("new-runtime")
            ]
        );
        assert!(!launch.environment.contains_key("STEAM_COMPAT_DATA_PATH"));
        assert!(!launch.environment.contains_key("LD_PRELOAD"));
    }
    #[test]
    fn handles_direct_proton_and_a_runner_without_a_container() {
        let f = Fixture::new();
        let before = f.command();
        let mut selected = f.choice();
        let direct = selected.launch(&before[7..], &BTreeMap::new()).unwrap();
        assert_eq!(direct.command[0], f.0.join("new-runtime/_v2-entry-point"));
        selected.runtime = None;
        let launch = selected.launch(&before, &BTreeMap::new()).unwrap();
        assert_eq!(&launch.command[..4], &before[..4]);
        assert_eq!(launch.command[4], f.0.join("new runner/proton"));
        assert_eq!(&launch.command[5..], &before[8..]);
    }
    #[test]
    fn refuses_native_commands_wrong_game_missing_runner_and_unknown_runtime_layout() {
        let f = Fixture::new();
        let selected = f.choice();
        assert!(selected
            .launch(&["/game/linux-native".into()], &BTreeMap::new())
            .is_err());
        assert!(selected
            .launch(
                &f.command(),
                &BTreeMap::from([("SteamAppId".into(), "99".into())])
            )
            .is_err());
        let mut broken = f.command();
        broken.insert(6, "--unknown".into());
        assert!(selected.launch(&broken, &BTreeMap::new()).is_err());
        let mut missing = selected;
        missing.directory = f.0.join("missing").to_string_lossy().into_owned();
        assert!(missing.launch(&f.command(), &BTreeMap::new()).is_err());
    }
}
