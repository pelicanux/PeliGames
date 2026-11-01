//! Durable version changes: restore originals through AMDNR's journal, then install the selected package.
//! Interrupted operations remain visible and can restore the previous runtime on the next launch.
use super::runtimes::Runtime;
use super::*;
use serde::{Deserialize, Serialize};
use std::{io::Write, os::unix::fs::OpenOptionsExt};
#[derive(Deserialize, Serialize)]
struct Pending {
    schema: u32,
    from: Runtime,
    to: Runtime,
    proxy: String,
    executable: String,
}
/// A package used for rollback must remain available until recovery finishes.
pub(super) fn ensure_package_removable(base: &Path, runtime: Runtime) -> Result<(), String> {
    let folder = base.join("version-changes");
    let meta = match fs::symlink_metadata(&folder) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err("Pasta de recuperação Daniel NR inválida.".into());
    }
    for (count, entry) in fs::read_dir(folder).map_err(|e| e.to_string())?.enumerate() {
        if count >= 10000 {
            return Err("Registros de recuperação demais para verificar a remoção.".into());
        }
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_none_or(|s| s != "json") {
            continue;
        }
        let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
            return Err("Registro de recuperação Daniel NR inválido.".into());
        }
        let p: Pending = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|_| "Recupere a troca interrompida antes de desinstalar pacotes.")?;
        if p.schema != 1 || p.from == runtime {
            return Err("Recupere a troca interrompida antes de desinstalar este pacote.".into());
        }
    }
    Ok(())
}
/// Keep the selected executable recoverable even between uninstall and reinstall.
/// Only an unfinished, bounded record inside this component can supply it.
pub(super) fn recoverable_game(base: &Path, game: &Game) -> Result<Game, String> {
    let mut recovered = game.clone();
    let root = Path::new(&game.directory)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let root = if root.is_file() {
        root.parent().ok_or("Jogo sem pasta.")?.to_path_buf()
    } else {
        root
    };
    let mut found = Vec::new();
    if let Ok(records) = fs::read_dir(base.join("version-changes")) {
        for record in records.take(1000).flatten() {
            let path = record.path();
            if path.extension().is_none_or(|s| s != "json") {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 4096 {
                continue;
            }
            let Ok(p) = serde_json::from_str::<Pending>(&text(&path)) else {
                continue;
            };
            let executable = PathBuf::from(&p.executable);
            if p.schema != 1 || !executable.starts_with(&root) || !pe64(&executable) {
                continue;
            }
            if game.selected_executable
                && game
                    .executable
                    .as_deref()
                    .is_some_and(|selected| Path::new(selected) != executable)
            {
                continue;
            }
            let Some(directory) = executable.parent() else {
                continue;
            };
            if !pending(base, directory) {
                continue;
            }
            found.push(executable);
        }
    }
    found.sort();
    found.dedup();
    if found.len() > 1 {
        return Err(
            "Há mais de uma troca interrompida. Escolha o executável do teste para recuperar."
                .into(),
        );
    }
    if let Some(executable) = found.pop() {
        recovered.executable = Some(string(&executable));
        recovered.selected_executable = true;
    }
    Ok(recovered)
}
fn path(base: &Path, directory: &Path, game: &Game) -> Result<PathBuf, String> {
    super::super::paths::names::allocate(
        &base.join("version-changes"),
        &game.name,
        &string(directory),
    )
    .map(|p| p.with_extension("json"))
}
pub fn pending(base: &Path, directory: &Path) -> bool {
    super::super::paths::names::find(&base.join("version-changes"), &string(directory))
        .is_some_and(|p| p.with_extension("json").exists())
}
fn settings(
    base: &Path,
    directory: &Path,
    game: &Game,
    runtime: Runtime,
) -> Result<PathBuf, String> {
    super::super::paths::names::allocate(
        &base.join("runtime-settings"),
        &format!("{}-{}", game.name, runtime.version()),
        &format!("{}:{}", string(directory), runtime.version()),
    )
}
const CONFIGS: &[&str] = &[
    "OptiScaler.ini",
    "AMDNR-NeuralHub-080/settings.ini",
    "AMDNR-NeuralHub-080/neuralhub.ini",
];
fn save_settings(
    base: &Path,
    directory: &Path,
    game: &Game,
    runtime: Runtime,
) -> Result<(), String> {
    let folder = settings(base, directory, game, runtime)?;
    for rel in CONFIGS {
        let src = directory.join(rel);
        if !src.exists() {
            continue;
        }
        let meta = fs::symlink_metadata(&src).map_err(|e| e.to_string())?;
        if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > 2 * 1024 * 1024 {
            return Err("Ajustes de runtime inválidos.".into());
        }
        let dst = folder.join(rel);
        fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
        fs::copy(&src, dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn restore_settings(
    base: &Path,
    directory: &Path,
    game: &Game,
    runtime: Runtime,
) -> Result<(), String> {
    let folder = settings(base, directory, game, runtime)?;
    for rel in CONFIGS {
        let src = folder.join(rel);
        let dst = directory.join(rel);
        if !src.exists() || !dst.is_file() {
            continue;
        }
        if src.metadata().map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024
            || fs::symlink_metadata(&src)
                .map_err(|e| e.to_string())?
                .file_type()
                .is_symlink()
        {
            return Err("Backup de ajustes inválido.".into());
        }
        fs::copy(src, dst).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn record(file: &Path, p: &Pending) -> Result<(), String> {
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(file)
        .map_err(|e| e.to_string())?;
    out.write_all(&serde_json::to_vec_pretty(p).map_err(|e| e.to_string())?)
        .and_then(|_| out.sync_all())
        .map_err(|e| e.to_string())
}
fn clean_install(
    base: &Path,
    game: &Game,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let (directory, executable) = target(game)?;
    if directory.join(JOURNAL).exists() {
        operations::remove(base, game, progress)?;
    } else if owned(&directory, &executable) {
        fs::remove_file(directory.join(RECEIPT)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
fn rollback(
    base: &Path,
    game: &Game,
    p: &Pending,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let (directory, _) = target(game)?;
    clean_install(base, game, progress)?;
    operations::install_version(base, game, true, &p.proxy, p.from, progress)?;
    restore_settings(base, &directory, game, p.from)
}
pub fn recover(
    base: &Path,
    game: &Game,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let recovered = recoverable_game(base, game)?;
    let game = &recovered;
    let (directory, exe) = target(game)?;
    ensure_closed(&exe)?;
    let file = path(base, &directory, game)?;
    let meta = fs::symlink_metadata(&file).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() || !meta.is_file() || meta.len() > 4096 {
        return Err("Registro de troca inválido.".into());
    }
    let p: Pending = serde_json::from_slice(&fs::read(&file).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    if p.schema != 1 {
        return Err("Registro de troca incompatível.".into());
    }
    if Path::new(&p.executable) != exe {
        return Err("Registro de troca pertence a outro executável.".into());
    }
    super::proxy::validate(&p.proxy, false)?;
    if !runtimes::ready(base, p.from) {
        return Err("Prepare a versão anterior antes de recuperar a troca.".into());
    }
    if p.from == Runtime::V08 {
        runtimes::verify(&p.from.package(base))?;
    }
    progress(format!("Recuperando Daniel {}…", p.from.version()));
    rollback(base, game, &p, progress)?;
    fs::remove_file(file).map_err(|e| e.to_string())
}
pub fn switch(
    base: &Path,
    game: &Game,
    runtime: Runtime,
    progress: &(dyn Fn(String) + Send + Sync),
) -> Result<(), String> {
    let (directory, exe) = target(game)?;
    ensure_closed(&exe)?;
    let current = runtimes::installed(&directory, &exe)
        .ok_or("Esta instalação não pertence ao teste Daniel NR.")?;
    if pending(base, &directory) {
        return Err("Há uma troca interrompida. Use Recuperar versão anterior.".into());
    }
    if current == runtime {
        return Ok(());
    }
    for r in [current, runtime] {
        if !runtimes::ready(base, r) {
            return Err(format!(
                "Prepare Daniel {} antes de trocar de versão.",
                r.version()
            ));
        }
        if r == Runtime::V08 {
            runtimes::verify(&r.package(base))?;
        }
    }
    let dll = proxy::state(&directory, true);
    if let Some(error) = dll.error {
        return Err(error);
    }
    let proxy = dll.current.ok_or("DLL atual não identificada.")?;
    save_settings(base, &directory, game, current)?;
    let file = path(base, &directory, game)?;
    let p = Pending {
        schema: 1,
        from: current,
        to: runtime,
        proxy,
        executable: string(&exe),
    };
    record(&file, &p)?;
    progress(format!(
        "Alterando Daniel {} para {}…",
        current.version(),
        runtime.version()
    ));
    if let Err(error) = operations::remove(base, game, progress) {
        // Keep the recovery record: the subprocess may have been interrupted after partial restoration.
        return Err(format!(
            "Remoção anterior não concluída. Use Recuperar versão anterior. {error}"
        ));
    }
    let result = (|| {
        operations::install_version(base, game, true, &p.proxy, runtime, progress)?;
        restore_settings(base, &directory, game, runtime)
    })();
    match result {
        Ok(()) => fs::remove_file(file).map_err(|e| e.to_string()),
        Err(error) => match rollback(base, game, &p, progress) {
            Ok(()) => {
                fs::remove_file(file).map_err(|e| e.to_string())?;
                Err(format!(
                    "A troca falhou; Daniel {} foi restaurado. {error}",
                    current.version()
                ))
            }
            Err(recovery) => Err(format!(
                "Troca interrompida: {error}\nUse Recuperar versão anterior. {recovery}"
            )),
        },
    }
}
