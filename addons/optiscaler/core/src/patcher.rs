//! Optional official ASI plugin, owned separately from OptiScaler and NR.
use super::{
    config_path, game_directory,
    installation::{hash, machine, safe_target},
    read_file, WRITE,
};
use peligames_mod_config::ini::{get, put, sections};
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, path::Path};
const PLUGIN: &str = "OptiScaler/plugins/OptiPatcher.asi";
const LICENSE: &str = "OptiScaler/Licenses/OptiPatcher_LICENSE.txt";
const RECEIPT: &str = "peligames-optipatcher-install.json";
#[derive(Serialize, Deserialize)]
struct Receipt {
    schema: u8,
    sha256: String,
    license_sha256: String,
}
#[derive(Serialize)]
pub struct State {
    pub installed: bool,
    pub enabled: bool,
    pub managed: bool,
}
pub fn state(game: &Path) -> Result<State, String> {
    let target = safe_target(game, PLUGIN)?;
    let ini = read_file(&config_path(game)?)?;
    let parsed = sections(&ini);
    Ok(State {
        installed: target.is_file(),
        enabled: target.is_file()
            && get(&parsed, "Plugins", "LoadAsiPlugins")
                .is_some_and(|s| s.eq_ignore_ascii_case("true")),
        managed: safe_target(game, RECEIPT)?.is_file(),
    })
}
fn validate_path(ini: &str) -> Result<(), String> {
    let parsed = sections(ini);
    if get(&parsed, "Plugins", "Path")
        .is_some_and(|s| !s.eq_ignore_ascii_case("auto") && !s.is_empty())
    {
        return Err("OptiScaler uses a custom plugin directory. Restore Plugins.Path=auto before managing OptiPatcher.".into());
    }
    if get(&parsed, "Libraries", "OptiDllPath")
        .is_some_and(|s| !s.eq_ignore_ascii_case("auto") && !s.is_empty())
    {
        return Err("OptiScaler uses a custom library directory. Restore Libraries.OptiDllPath=auto before managing OptiPatcher.".into());
    }
    Ok(())
}
pub fn enable(game: &Path, component: &Path) -> Result<(), String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    let ini_path = config_path(&game)?;
    let ini = read_file(&ini_path)?;
    if ini.is_empty() {
        return Err("Install OptiScaler first".into());
    }
    validate_path(&ini)?;
    let source = component.join("OptiPatcher.asi");
    if machine(&source) != Some(0x8664) {
        return Err("Invalid 64-bit OptiPatcher plugin".into());
    }
    let target = safe_target(&game, PLUGIN)?;
    let license = safe_target(&game, LICENSE)?;
    let receipt = safe_target(&game, RECEIPT)?;
    if target.exists() || license.exists() || receipt.exists() {
        return Err("OptiPatcher files already exist. Remove the existing installation before replacing it.".into());
    }
    let mut written = Vec::new();
    let result = (|| {
        for (src, dst) in [
            (source, target.clone()),
            (component.join("OptiPatcher_LICENSE.txt"), license.clone()),
        ] {
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            let mut out = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&dst)
                .map_err(|e| e.to_string())?;
            written.push(dst);
            let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
            std::io::copy(&mut input, &mut out).map_err(|e| e.to_string())?;
            out.sync_all().map_err(|e| e.to_string())?;
        }
        let info = Receipt {
            schema: 1,
            sha256: hash(&target)?,
            license_sha256: hash(&license)?,
        };
        let mut out = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&receipt)
            .map_err(|e| e.to_string())?;
        written.push(receipt);
        out.write_all(&serde_json::to_vec_pretty(&info).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        out.sync_all().map_err(|e| e.to_string())?;
        peligames_mod_config::write_atomic(
            &ini_path,
            &put(&ini, "Plugins", "LoadAsiPlugins", Some("true")),
        )
    })();
    if result.is_err() {
        for path in written {
            let _ = fs::remove_file(path);
        }
    }
    result
}
/// Caller holds WRITE. Disabling this plugin never disables the shared ASI loader.
pub(super) fn remove_owned(game: &Path) -> Result<(), String> {
    let receipt = safe_target(game, RECEIPT)?;
    if !receipt.exists() {
        return Ok(());
    }
    let info: Receipt = serde_json::from_str(&read_file(&receipt)?).map_err(|e| e.to_string())?;
    if info.schema != 1 {
        return Err("Invalid OptiPatcher receipt".into());
    }
    let files = [
        (safe_target(game, PLUGIN)?, info.sha256),
        (safe_target(game, LICENSE)?, info.license_sha256),
    ];
    for (path, expected) in &files {
        if path.exists() && hash(path)? != *expected {
            return Err("OptiPatcher was changed outside PeliGames; removal stopped".into());
        }
    }
    for (path, _) in files {
        if path.exists() {
            fs::remove_file(path).map_err(|e| e.to_string())?;
        }
    }
    fs::remove_file(receipt).map_err(|e| e.to_string())
}
pub fn disable(game: &Path) -> Result<(), String> {
    let _lock = WRITE.lock().map_err(|e| e.to_string())?;
    let game = game_directory(game)?;
    let _directory = peligames_mod_config::DirectoryLock::acquire(&game)?;
    if !safe_target(&game, RECEIPT)?.exists() && safe_target(&game, PLUGIN)?.exists() {
        return Err(
            "This OptiPatcher was installed externally; its files will not be removed".into(),
        );
    }
    remove_owned(&game)
}
