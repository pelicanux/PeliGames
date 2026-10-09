//! Unpack the documented Windows install.dat payload; never run installer scripts.
use super::{
    nexus_archive::{safe_name, FILE_LIMIT, TOTAL_LIMIT},
    nexus_local::{LocalMod, NexusGame},
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Cursor, Read, Write},
};
const ROOT_FILES: &[&str] = &[
    "StardewModdingAPI.exe",
    "StardewModdingAPI.dll",
    "StardewModdingAPI.exe.config",
    "StardewModdingAPI.runtimeconfig.json",
    "StardewModdingAPI.xml",
    "steam_appid.txt",
];
fn game_deps(game: &NexusGame) -> Result<Vec<u8>, String> {
    let root = fs::canonicalize(
        game.game["directory"]
            .as_str()
            .ok_or("Pasta do jogo inválida.")?,
    )
    .map_err(|e| e.to_string())?;
    let path = fs::canonicalize(root.join("Stardew Valley.deps.json")).map_err(|_| "Falta Stardew Valley.deps.json na instalação do jogo; verifique os arquivos do Stardew Valley.")?;
    if !path.starts_with(&root) {
        return Err("O arquivo de dependências do jogo aponta para fora da sua pasta.".into());
    }
    let file = fs::File::open(path).map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Dependências do jogo inválidas.".into());
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("Dependências do jogo excedem 16 MiB.".into());
    }
    let json: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|_| "Stardew Valley.deps.json não contém JSON válido.")?;
    if !json["runtimeTarget"].is_object() || !json["libraries"].is_object() {
        return Err(
            "Dependências do jogo não correspondem ao formato .NET esperado pelo SMAPI.".into(),
        );
    }
    Ok(bytes)
}
pub(super) fn prepare(
    game: &NexusGame,
    item: &LocalMod,
    mut archive: zip::ZipArchive<fs::File>,
) -> Result<zip::ZipArchive<fs::File>, String> {
    if game.adapter != "stardewvalley" || game.platform != "proton" {
        return Ok(archive);
    }
    let mut candidates = Vec::new();
    let mut installer = false;
    for i in 0..archive.len() {
        let file = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = file.name().replace('\\', "/");
        let lower = name.to_ascii_lowercase();
        if [
            "internal/windows/install.dat",
            "internal/linux/install.dat",
            "internal/macos/install.dat",
            "internal/unix/install.dat",
        ]
        .iter()
        .any(|suffix| lower == *suffix || lower.ends_with(&format!("/{suffix}")))
        {
            installer = true;
        }
        if lower == "internal/windows/install.dat"
            || lower.ends_with("/internal/windows/install.dat")
        {
            candidates.push(i);
        }
    }
    if candidates.is_empty() {
        if installer {
            return Err("O instalador SMAPI não contém internal/windows/install.dat para este jogo Windows/Proton.".into());
        }
        return Ok(archive);
    }
    if candidates.len() != 1 {
        return Err(
            "O pacote contém mais de um instalador Windows do SMAPI; escolha uma versão.".into(),
        );
    }
    let mut file = archive.by_index(candidates[0]).map_err(|e| e.to_string())?;
    if file.size() > FILE_LIMIT {
        return Err("O pacote interno SMAPI excede 512 MiB.".into());
    }
    let mut payload = Vec::new();
    let size = file.size();
    (&mut file)
        .take(FILE_LIMIT + 1)
        .read_to_end(&mut payload)
        .map_err(|e| e.to_string())?;
    if payload.len() as u64 != size {
        return Err("Tamanho inválido do pacote interno SMAPI.".into());
    }
    drop(file);
    let deps = game_deps(game)?;
    let mut nested = zip::ZipArchive::new(Cursor::new(&payload))
        .map_err(|_| "install.dat do SMAPI não é um ZIP válido.")?;
    if nested.len() > 10000 {
        return Err("Entradas demais no pacote interno SMAPI.".into());
    }
    let mut seen = BTreeSet::new();
    let mut total = 0u64;
    for i in 0..nested.len() {
        let mut entry = nested.by_index(i).map_err(|e| e.to_string())?;
        let name = safe_name(entry.name())?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("Links não são aceitos no pacote SMAPI.".into());
        }
        if entry.is_dir() {
            continue;
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(format!("Arquivo duplicado no SMAPI: {name}"));
        }
        total = total
            .checked_add(entry.size())
            .ok_or("Pacote SMAPI grande demais.")?;
        if entry.size() > FILE_LIMIT || total > TOTAL_LIMIT {
            return Err("O pacote SMAPI excede os limites de extração.".into());
        }
        if !ROOT_FILES.contains(&name.as_str())
            && !name.starts_with("smapi-internal/")
            && !name.starts_with("Mods/ConsoleCommands/")
            && !name.starts_with("Mods/SaveBackup/")
        {
            return Err(format!("Conteúdo inesperado no pacote SMAPI: {name}"));
        }
        if ["StardewModdingAPI.exe", "StardewModdingAPI.dll"].contains(&name.as_str()) {
            let mut header = [0; 2];
            entry
                .read_exact(&mut header)
                .map_err(|_| "Executável SMAPI incompleto.")?;
            if header != *b"MZ" {
                return Err("Este pacote não contém o SMAPI para Windows.".into());
            }
        }
        if name.ends_with("/manifest.json") {
            let expected = if name.starts_with("Mods/ConsoleCommands/") {
                Some("SMAPI.ConsoleCommands")
            } else if name.starts_with("Mods/SaveBackup/") {
                Some("SMAPI.SaveBackup")
            } else {
                None
            };
            if let Some(expected) = expected {
                let mut manifest = Vec::new();
                entry
                    .take(1024 * 1024 + 1)
                    .read_to_end(&mut manifest)
                    .map_err(|e| e.to_string())?;
                if manifest.len() > 1024 * 1024 {
                    return Err("Manifesto SMAPI grande demais.".into());
                }
                let value: serde_json::Value =
                    serde_json::from_slice(&manifest).map_err(|_| "Manifesto SMAPI inválido.")?;
                if value.as_object().and_then(|object| {
                    object
                        .iter()
                        .find(|(key, _)| key.eq_ignore_ascii_case("UniqueID"))
                        .and_then(|(_, id)| id.as_str())
                }) != Some(expected)
                {
                    return Err("Mod integrado ao SMAPI não reconhecido.".into());
                }
            }
        }
    }
    for required in [
        "StardewModdingAPI.exe",
        "StardewModdingAPI.dll",
        "StardewModdingAPI.runtimeconfig.json",
        "smapi-internal/config.json",
    ] {
        if !seen.contains(&required.to_ascii_lowercase()) {
            return Err(format!("Instalador SMAPI incompleto: falta {required}."));
        }
    }
    let mut hash = Sha256::new();
    hash.update(b"smapi-windows-v1");
    hash.update(&payload);
    hash.update(&deps);
    let cache = super::nexus_profile::directory(game)?.join("archive-cache");
    fs::create_dir_all(&cache).map_err(|e| e.to_string())?;
    let target = cache.join(format!("{}-{:x}.zip", item.id, hash.finalize()));
    if !target.is_file() {
        let temporary = cache.join(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut output =
                zip::ZipWriter::new(fs::File::create(&temporary).map_err(|e| e.to_string())?);
            for i in 0..nested.len() {
                let mut entry = nested.by_index(i).map_err(|e| e.to_string())?;
                if entry.is_dir() {
                    continue;
                }
                let name = safe_name(entry.name())?;
                let expected = entry.size();
                output
                    .start_file(name, zip::write::FileOptions::default())
                    .map_err(|e| e.to_string())?;
                let copied = std::io::copy(&mut (&mut entry).take(expected + 1), &mut output)
                    .map_err(|e| e.to_string())?;
                if copied != expected {
                    return Err("Tamanho inválido na extração do SMAPI.".to_string());
                }
            }
            output
                .start_file(
                    "StardewModdingAPI.deps.json",
                    zip::write::FileOptions::default(),
                )
                .map_err(|e| e.to_string())?;
            output.write_all(&deps).map_err(|e| e.to_string())?;
            output
                .finish()
                .map_err(|e| e.to_string())?
                .sync_all()
                .map_err(|e| e.to_string())?;
            fs::rename(&temporary, &target).map_err(|e| e.to_string())
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(temporary);
            return Err(error);
        }
    }
    zip::ZipArchive::new(fs::File::open(target).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
