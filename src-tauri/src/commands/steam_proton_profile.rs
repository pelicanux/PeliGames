//! Resolve the runtime required by a per-game Proton without changing Steam VDFs.
use super::external_proton::vdf;
use crate::commands::steam_runtime::contract::steam_proton::SteamProton;
use std::{
    fs,
    path::{Path, PathBuf},
};

fn document(path: &Path) -> Result<serde_json::Value, String> {
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("Manifesto do Proton grande demais.".into());
    }
    vdf(&fs::read_to_string(path).map_err(|e| e.to_string())?)
}
fn field<'a>(value: &'a serde_json::Value, key: &str) -> Option<&'a serde_json::Value> {
    value
        .as_object()?
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, v)| v)
}
fn libraries(home: &Path) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for root in super::super::steam_installation::roots(home, false) {
        if !result.contains(&root) {
            result.push(root.clone());
        }
        for name in ["steamapps/libraryfolders.vdf", "config/libraryfolders.vdf"] {
            if let Ok(value) = document(&root.join(name)) {
                if let Some(folders) = field(&value, "libraryfolders").and_then(|v| v.as_object()) {
                    for folder in folders.values() {
                        if let Some(path) = field(folder, "path")
                            .and_then(|p| p.as_str())
                            .map(PathBuf::from)
                            .filter(|p| p.is_absolute())
                        {
                            if !result.contains(&path) {
                                result.push(path);
                            }
                        }
                    }
                }
            }
        }
    }
    result
}
pub(in crate::commands) fn resolve(home: &Path, app: u32, proton: &str) -> Result<SteamProton, String> {
    let runner = crate::core::game_installation::validate_proton(proton)?;
    if !runner.join("proton").is_file() {
        return Err(
            "Escolha uma versão Proton; Wine avulso não é compatível com jogos da Steam.".into(),
        );
    }
    let data = document(&runner.join("toolmanifest.vdf")).map_err(|e| {
        format!("Não foi possível verificar o runtime exigido por este Proton: {e}")
    })?;
    let manifest = field(&data, "manifest").ok_or("Manifesto do Proton inválido.")?;
    let command = field(manifest, "commandline").and_then(|v| v.as_str());
    if !matches!(command, Some("/proton %verb%" | "./proton %verb%")) {
        return Err("Este Proton usa um comando de inicialização ainda não suportado pelo perfil. Selecione-o diretamente na Steam.".into());
    }
    let required = field(manifest, "require_tool_appid")
        .and_then(|v| v.as_str())
        .filter(|id| *id != "0");
    let runtime = if let Some(id) = required {
        if id.is_empty() || !id.bytes().all(|c| c.is_ascii_digit()) {
            return Err("Identificação do Steam Linux Runtime inválida.".into());
        }
        let mut found = None;
        for library in libraries(home) {
            let Ok(data) = document(&library.join(format!("steamapps/appmanifest_{id}.acf")))
            else {
                continue;
            };
            let state = field(&data, "AppState").ok_or("Manifesto do runtime inválido.")?;
            if field(state, "appid").and_then(|v| v.as_str()) != Some(id) {
                continue;
            }
            let Some(folder) = field(state, "installdir").and_then(|v| v.as_str()) else {
                continue;
            };
            let relative = Path::new(folder);
            if relative.components().count() != 1
                || !matches!(
                    relative.components().next(),
                    Some(std::path::Component::Normal(_))
                )
            {
                continue;
            }
            let root = library.join("steamapps/common");
            let candidate = root.join(relative);
            if candidate.join("_v2-entry-point").is_file() {
                found = Some(
                    candidate
                        .canonicalize()
                        .map_err(|e| e.to_string())?
                        .to_string_lossy()
                        .into_owned(),
                );
                break;
            }
        }
        Some(found.ok_or_else(||format!("Instale o Steam Linux Runtime exigido por este Proton (AppID {id}) pela Steam e tente novamente."))?)
    } else {
        None
    };
    let choice = SteamProton {
        app_id: app,
        directory: runner.to_string_lossy().into_owned(),
        runtime,
    };
    choice.validate()?;
    Ok(choice)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    #[test]
    fn resolves_required_runtime_on_another_disk_without_modifying_steam() {
        let root =
            std::env::temp_dir().join(format!("peli-proton-resolution-{}", uuid::Uuid::new_v4()));
        let steam = root.join(".local/share/Steam");
        let other = root.join("another disk");
        let runner = root.join("Proton");
        fs::create_dir_all(steam.join("steamapps")).unwrap();
        fs::create_dir_all(other.join("steamapps/common/SteamLinuxRuntime_test")).unwrap();
        fs::create_dir_all(&runner).unwrap();
        fs::write(runner.join("proton"), "#!/bin/sh\n").unwrap();
        fs::set_permissions(runner.join("proton"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(
            runner.join("toolmanifest.vdf"),
            r#""manifest" { "commandline" "/proton %verb%" "require_tool_appid" "123" }"#,
        )
        .unwrap();
        let config = format!(
            r#""libraryfolders" {{ "0" {{ "path" "{}" }} }}"#,
            other.display()
        );
        fs::write(steam.join("steamapps/libraryfolders.vdf"), &config).unwrap();
        fs::write(
            other.join("steamapps/appmanifest_123.acf"),
            r#""AppState" { "appid" "123" "installdir" "SteamLinuxRuntime_test" }"#,
        )
        .unwrap();
        fs::write(
            other.join("steamapps/common/SteamLinuxRuntime_test/_v2-entry-point"),
            "#!/bin/sh\n",
        )
        .unwrap();
        let choice = resolve(&root, 42, runner.to_str().unwrap()).unwrap();
        assert_eq!(choice.app_id, 42);
        assert_eq!(
            choice.runtime.as_deref(),
            Some(
                other
                    .join("steamapps/common/SteamLinuxRuntime_test")
                    .to_str()
                    .unwrap()
            )
        );
        assert_eq!(
            fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")).unwrap(),
            config
        );
        fs::remove_file(other.join("steamapps/common/SteamLinuxRuntime_test/_v2-entry-point"))
            .unwrap();
        assert!(resolve(&root, 42, runner.to_str().unwrap())
            .unwrap_err()
            .contains("123"));
        fs::write(
            other.join("steamapps/appmanifest_123.acf"),
            r#""AppState" { "appid" "123" "installdir" "../outside" }"#,
        )
        .unwrap();
        assert!(resolve(&root, 42, runner.to_str().unwrap()).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
