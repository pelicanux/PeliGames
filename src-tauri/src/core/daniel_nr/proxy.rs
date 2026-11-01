//! Loading aliases supported by the pinned AMDNR-Linux package.
use super::*;
use std::os::unix::fs::OpenOptionsExt;

const SUPPORTED: &[&str] = &[
    "winmm.dll",
    "version.dll",
    "dbghelp.dll",
    "wininet.dll",
    "winhttp.dll",
];

#[derive(Serialize)]
pub struct LoadingDllOption {
    pub name: &'static str,
    pub available: bool,
}
#[derive(Serialize)]
pub struct LoadingDllState {
    pub current: Option<String>,
    pub options: Vec<LoadingDllOption>,
    pub error: Option<String>,
}

pub(super) fn validate(name: &str, allow_auto: bool) -> Result<(), String> {
    if SUPPORTED.contains(&name) || (allow_auto && name == "auto") {
        Ok(())
    } else {
        Err("DLL de carregamento inválida. Escolha uma das opções suportadas.".into())
    }
}

fn current(directory: &Path) -> Result<String, String> {
    let bundle = directory.join("amdnr-linux");
    if !fs::symlink_metadata(&bundle).is_ok_and(|m| m.is_dir() && !m.file_type().is_symlink()) {
        return Err("A pasta da ponte está ausente ou não é um diretório regular.".into());
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(bundle.join("proxy.json"));
    let file = match file {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok("winmm.dll".into()),
        Err(_) => return Err("Não foi possível ler a DLL de carregamento instalada.".into()),
    };
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() <= 4096)
    {
        return Err("Registro da DLL de carregamento inválido.".into());
    }
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Record {
        proxy: String,
    }
    let record: Record = serde_json::from_reader(file.take(4097))
        .map_err(|_| "Registro da DLL de carregamento inválido.")?;
    validate(&record.proxy, false)?;
    Ok(record.proxy)
}

pub(super) fn state(directory: &Path, installed: bool) -> LoadingDllState {
    let (current, mut error) = if installed {
        match current(directory) {
            Ok(name) => (Some(name), None),
            Err(error) => (None, Some(error)),
        }
    } else {
        (None, None)
    };
    let entries =
        fs::read_dir(directory).and_then(|entries| entries.collect::<Result<Vec<_>, _>>());
    if entries.is_err() {
        error = Some("Não foi possível verificar os nomes de DLL disponíveis.".into());
    }
    let options = SUPPORTED
        .iter()
        .map(|&name| {
            let available = entries.as_ref().is_ok_and(|entries| {
                let mut matches = entries.iter().filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(name)
                });
                match (matches.next(), matches.next()) {
                    (None, _) => true,
                    (Some(entry), None) => {
                        current.as_deref() == Some(name)
                            && entry.file_name() == name
                            && entry.file_type().is_ok_and(|kind| kind.is_file())
                    }
                    _ => false,
                }
            });
            LoadingDllOption { name, available }
        })
        .collect();
    LoadingDllState {
        current,
        options,
        error,
    }
}
