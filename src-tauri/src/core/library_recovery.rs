//! Recover launcher-owned manifests from user-selected folders, without executable discovery.
use super::*;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
const MAX_MANIFEST: u64 = 2 * 1024 * 1024;
const MAX_INSTALLATIONS: usize = 512;
#[derive(Clone, Serialize, serde::Deserialize)]
pub struct Candidate {
    pub directory: String,
    pub name: String,
    pub kind: String,
    pub games: usize,
    pub registered: bool,
    pub warnings: Vec<String>,
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct Discovery {
    pub token: String,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<String>,
}
struct Found {
    directory: PathBuf,
    original: Vec<u8>,
    record: InstallationRecord,
    copy: Option<Vec<u8>>,
}
struct Plan {
    token: String,
    found: Vec<Found>,
    candidates: Vec<Candidate>,
    map: Vec<(PathBuf, PathBuf)>,
}
static PLAN: OnceLock<Mutex<Option<Plan>>> = OnceLock::new();
fn read(path: &Path) -> Result<Vec<u8>, String> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| e.to_string())?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("Registro não é um arquivo regular.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_MANIFEST {
        return Err("Registro excede o limite permitido.".into());
    }
    Ok(bytes)
}
fn translate(value: &str, map: &[(PathBuf, PathBuf)]) -> String {
    if value.is_empty() || !Path::new(value).is_absolute() {
        return value.into();
    }
    map.iter()
        .find_map(|(old, new)| {
            Path::new(value).strip_prefix(old).ok().map(|rest| {
                if rest.as_os_str().is_empty() {
                    new.to_string_lossy().into_owned()
                } else {
                    new.join(rest).to_string_lossy().into_owned()
                }
            })
        })
        .unwrap_or_else(|| value.into())
}
fn valid_path(value: &str) -> bool {
    !value.is_empty()
        && Path::new(value).is_absolute()
        && !Path::new(value)
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
}
fn relocated(
    record: &InstallationRecord,
    map: &[(PathBuf, PathBuf)],
) -> Result<InstallationRecord, String> {
    let mut r: InstallationRecord =
        serde_json::from_value(serde_json::to_value(record).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    let t = |s: &mut String| *s = translate(s, map);
    t(&mut r.directory);
    t(&mut r.prefix);
    t(&mut r.installer);
    if let Some(v) = &mut r.selected_executable {
        t(v);
    }
    for value in r.baseline.iter_mut().chain(r.launcher_games.iter_mut()) {
        t(value);
    }
    for values in [&mut r.known_shortcuts, &mut r.install_shortcuts]
        .into_iter()
        .flatten()
    {
        for value in values {
            t(value);
        }
    }
    for e in &mut r.entries {
        for s in [
            &mut e.path,
            &mut e.directory,
            &mut e.executable,
            &mut e.prefix,
        ] {
            t(s);
        }
        for arg in &mut e.launch_arguments {
            if Path::new(arg).is_absolute() {
                t(arg);
            }
        }
        if let Some(p) = &mut e.profile {
            t(&mut p.root);
            t(&mut p.base);
        }
        // Runtime wrapper paths and launch commands belong to the previous system.
        e.advanced.wrappers.clear();
        e.advanced.launch_options = None;
    }
    for o in &mut r.overrides {
        t(&mut o.source);
        t(&mut o.executable);
        if let Some(prefix) = &mut o.prefix {
            t(prefix);
        }
        if let Some(a) = &mut o.advanced {
            a.wrappers.clear();
            a.launch_options = None;
        }
    }
    Ok(r)
}
fn examine(found: &Found, map: &[(PathBuf, PathBuf)], known: &BTreeSet<String>) -> Candidate {
    let mut item = Candidate {
        directory: found.directory.to_string_lossy().into_owned(),
        name: found.record.name.clone(),
        kind: "Instalação".into(),
        games: found.record.entries.len(),
        registered: known.contains(&found.directory.to_string_lossy().into_owned()),
        warnings: vec![],
        error: None,
    };
    let inspect = || -> Result<(), String> {
        let r = relocated(&found.record, map)?;
        if Path::new(&r.directory) != found.directory || !valid_path(&r.prefix) {
            return Err("Registro contém locais inválidos ou ambíguos.".into());
        }
        if r.entries.is_empty() {
            return Err(
                "Instalação sem jogos cadastrados; pode ter sido removida da biblioteca.".into(),
            );
        }
        let mut overlay = false;
        for e in &r.entries {
            if e.launcher != "PeliGames" || !valid_path(&e.executable) || !valid_path(&e.prefix) {
                return Err("Registro não corresponde a uma instalação PeliGames válida.".into());
            }
            if let Some(p) = &e.profile {
                if !valid_path(&p.root)
                    || !Path::new(&p.root).starts_with(&found.directory)
                    || !Path::new(&p.root).is_dir()
                {
                    return Err("Pasta do perfil não encontrada nesta instalação.".into());
                }
                if p.kind == super::super::game_profiles::ProfileKind::Overlay {
                    overlay = true;
                    if !valid_path(&p.base) || !Path::new(&p.base).join("drive_c").is_dir() {
                        return Err("Base do overlay não encontrada. Inclua também a pasta da instalação original na busca.".into());
                    }
                    if !Path::new(&p.root).join("upper").is_dir() {
                        return Err("Pasta de alterações do overlay não encontrada.".into());
                    }
                    if !Path::new(&e.executable).starts_with(p.prefix()) {
                        return Err("Executável do overlay está fora do próprio perfil.".into());
                    }
                } else if !Path::new(&e.executable).is_file() {
                    return Err("Executável do perfil não encontrado.".into());
                }
            } else if !Path::new(&e.executable).is_file() || !Path::new(&e.prefix).is_dir() {
                return Err(
                    "Executável ou prefixo não encontrado. Preserve todas as pastas da instalação."
                        .into(),
                );
            }
        }
        if !overlay && !Path::new(&r.prefix).is_dir() {
            return Err("Prefixo da instalação não encontrado.".into());
        }
        Ok(())
    };
    item.error = inspect().err();
    if found.record.entries.iter().any(|e| {
        e.profile
            .as_ref()
            .is_some_and(|p| p.kind == super::super::game_profiles::ProfileKind::Overlay)
    }) {
        item.kind = "Overlay".into();
    } else if let Some(bytes) = &found.copy {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(bytes) {
            item.kind = match v["kind"].as_str() {
                Some("reflink") => "Reflink",
                Some("full") => "Cópia completa",
                _ => "Cópia",
            }
            .into();
        }
    }
    if found.record.directory != item.directory {
        item.warnings
            .push("O local mudou; as referências registradas serão atualizadas.".into());
    }
    if !found.record.proton.is_empty() && !Path::new(&found.record.proton).is_dir() {
        item.warnings.push(
            "O Proton anterior não está disponível. Escolha outro nos detalhes após importar."
                .into(),
        );
    }
    if found.record.entries.iter().any(|e| {
        !e.advanced.wrappers.is_empty()
            || e.advanced
                .launch_options
                .as_ref()
                .is_some_and(|s| !s.is_empty())
    }) {
        item.warnings.push(
            "Comandos e wrappers da instalação anterior serão desativados; configure-os novamente."
                .into(),
        );
    }
    item
}
pub fn discover(roots: Vec<String>) -> Result<Discovery, String> {
    discover_at(&library_file()?, roots)
}
fn discover_at(index: &Path, roots: Vec<String>) -> Result<Discovery, String> {
    if roots.is_empty() || roots.len() > 8 {
        return Err("Selecione de uma a oito pastas de biblioteca.".into());
    }
    let mut stack = Vec::new();
    let mut visited = BTreeSet::new();
    let mut found = Vec::new();
    let mut warnings = Vec::new();
    let mut traversed = 0;
    let mut bytes_read = 0;
    for root in roots {
        let root = PathBuf::from(root)
            .canonicalize()
            .map_err(|_| "Pasta de biblioteca indisponível.")?;
        if !root.is_dir() {
            return Err("Selecione uma pasta.".into());
        }
        stack.push((root, 0));
    }
    while let Some((folder, depth)) = stack.pop() {
        if !visited.insert(folder.clone()) {
            continue;
        }
        traversed += 1;
        if traversed > 20000 || found.len() >= MAX_INSTALLATIONS {
            warnings.push(
                "Busca limitada. Escolha pastas mais específicas para encontrar o restante.".into(),
            );
            break;
        }
        let manifest = folder.join("installation.json");
        if !manifest.exists() && folder.join(".peligames/installation.json").is_file() {
            warnings.push(format!("{} usa o formato antigo .peligames. Atualize essa instalação pelo PeliGames anterior antes de importar; os arquivos foram preservados.",folder.display()));
            continue;
        }
        if manifest.exists() {
            match read(&manifest).and_then(|bytes| {
                serde_json::from_slice::<InstallationRecord>(&bytes)
                    .map(|record| (bytes, record))
                    .map_err(|e| e.to_string())
            }) {
                Ok((original, record)) if valid_path(&record.directory) => {
                    let copy = if folder.join("copy.json").exists() {
                        match read(&folder.join("copy.json")) {
                            Ok(bytes) => Some(bytes),
                            Err(_) => {
                                warnings.push(format!(
                                    "Registro de cópia inválido em {}.",
                                    folder.display()
                                ));
                                continue;
                            }
                        }
                    } else {
                        None
                    };
                    bytes_read += original.len() + copy.as_ref().map_or(0, Vec::len);
                    if bytes_read > 32 * 1024 * 1024 {
                        warnings.push(
                            "Os registros excedem o limite da busca; selecione uma pasta menor."
                                .into(),
                        );
                        break;
                    }
                    found.push(Found {
                        directory: folder,
                        original,
                        record,
                        copy,
                    });
                }
                _ => warnings.push(format!(
                    "Registro inválido em {}. Arquivos preservados.",
                    folder.display()
                )),
            }
            continue;
        }
        if depth >= 6 {
            continue;
        }
        let entries = match fs::read_dir(&folder) {
            Ok(e) => e,
            Err(_) => {
                warnings.push(format!("Sem acesso a {}.", folder.display()));
                continue;
            }
        };
        for entry in entries.flatten().take(20000) {
            if stack.len() >= 20000 {
                warnings.push("Pasta grande demais. Selecione uma subpasta da biblioteca.".into());
                break;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if [
                ".peligames",
                "prefix",
                "pfx",
                "drive_c",
                "dosdevices",
                "upper",
                "work",
                "node_modules",
                ".git",
            ]
            .contains(&name.as_ref())
            {
                continue;
            }
            if entry
                .file_type()
                .is_ok_and(|m| m.is_dir() && !m.is_symlink())
            {
                stack.push((entry.path(), depth + 1));
            }
        }
    }
    found.sort_by(|a, b| a.directory.cmp(&b.directory));
    let mut mappings = BTreeMap::<PathBuf, PathBuf>::new();
    let mut ambiguous = BTreeSet::new();
    for f in &found {
        let old = PathBuf::from(&f.record.directory);
        if mappings
            .insert(old.clone(), f.directory.clone())
            .is_some_and(|previous| previous != f.directory)
        {
            ambiguous.insert(old);
        }
    }
    if !ambiguous.is_empty() {
        return Err("Foram encontrados registros duplicados da mesma instalação original. Selecione apenas a biblioteca que deseja recuperar.".into());
    }
    let mut map: Vec<_> = mappings.into_iter().collect();
    map.sort_by_key(|(old, _)| std::cmp::Reverse(old.components().count()));
    let known = destinations(index)?;
    let candidates=found.iter().map(|f|{let mut c=examine(f,&map,&known);if ambiguous.contains(Path::new(&f.record.directory)){c.error=Some("Mais de uma pasta usa o mesmo registro original. Busque apenas a cópia que deseja recuperar.".into());}c}).collect::<Vec<_>>();
    let mut candidates = candidates;
    for (item_found, item) in found.iter().zip(candidates.iter_mut()) {
        if item.kind != "Overlay" || item.error.is_some() {
            continue;
        }
        if let Ok(record) = relocated(&item_found.record, &map) {
            for profile in record
                .entries
                .iter()
                .filter_map(|entry| entry.profile.as_ref())
                .filter(|p| p.kind == super::super::game_profiles::ProfileKind::Overlay)
            {
                if let Some(base) = found
                    .iter()
                    .find(|base| translate(&base.record.prefix, &map) == profile.base)
                {
                    item.warnings.push(format!(
                        "Inclua também a instalação base: {}.",
                        base.record.name
                    ));
                }
            }
        }
    }
    let token = uuid::Uuid::new_v4().to_string();
    let response = Discovery {
        token: token.clone(),
        candidates: candidates.clone(),
        warnings,
    };
    *PLAN
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())? = Some(Plan {
        token,
        found,
        candidates,
        map,
    });
    Ok(response)
}
#[derive(Debug, Serialize)]
pub struct Imported {
    pub installations: usize,
    pub games: usize,
}
pub fn restore(token: String, selected: Vec<String>) -> Result<Imported, String> {
    restore_at(&library_file()?, token, selected)
}
fn restore_at(index: &Path, token: String, selected: Vec<String>) -> Result<Imported, String> {
    let mut cache = PLAN
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|e| e.to_string())?;
    let plan = cache
        .as_ref()
        .filter(|p| p.token == token)
        .ok_or("A busca expirou. Localize a biblioteca novamente.")?;
    let chosen: BTreeSet<_> = selected.into_iter().collect();
    if chosen.is_empty() {
        return Err("Selecione uma instalação.".into());
    }
    if chosen.iter().any(|d| {
        !plan
            .candidates
            .iter()
            .any(|c| &c.directory == d && !c.registered && c.error.is_none())
    }) {
        return Err("Seleção inválida ou já cadastrada. Faça uma nova busca.".into());
    }
    let _guard = LIBRARY_LOCK.lock().map_err(|e| e.to_string())?;
    let _process = process_lock(index)?;
    let mut roots = destinations(index)?;
    let before_roots = roots.clone();
    let mut writes = Vec::<(PathBuf, Vec<u8>, Vec<u8>)>::new();
    let mut leases = Vec::new();
    let mut games = 0;
    let mut locked = BTreeSet::new();
    for f in &plan.found {
        if !chosen.contains(&f.directory.to_string_lossy().into_owned()) {
            continue;
        }
        if f.directory.canonicalize().map_err(|e| e.to_string())? != f.directory {
            return Err("O local da instalação mudou. Faça uma nova busca.".into());
        }
        if roots.contains(&f.directory.to_string_lossy().into_owned()) {
            return Err("Uma instalação já foi cadastrada. Faça uma nova busca.".into());
        }
        let manifest = f.directory.join("installation.json");
        if read(&manifest)? != f.original {
            return Err("Uma instalação mudou durante a busca. Procure novamente.".into());
        }
        let checked = examine(f, &plan.map, &roots);
        if let Some(error) = checked.error {
            return Err(error);
        }
        let record = relocated(&f.record, &plan.map)?;
        for entry in &record.entries {
            let lock_root = entry
                .profile
                .as_ref()
                .map(|p| p.root.as_str())
                .unwrap_or(&entry.prefix);
            if locked.insert(lock_root.to_owned()) {
                leases.push(super::super::game_installation::lock_prefix(Path::new(
                    lock_root,
                ))?);
            }
            if super::super::game_execution::prefix_in_use(Path::new(&entry.prefix)) {
                return Err(
                    "Feche os jogos e ferramentas Wine desta biblioteca antes de importar.".into(),
                );
            }
            if let Some(profile) = &entry.profile {
                if profile.kind == super::super::game_profiles::ProfileKind::Overlay {
                    if let Some(base) = plan
                        .found
                        .iter()
                        .find(|base| translate(&base.record.prefix, &plan.map) == profile.base)
                    {
                        let base_directory = base.directory.to_string_lossy().into_owned();
                        if !chosen.contains(&base_directory) && !roots.contains(&base_directory) {
                            return Err("Selecione também a instalação base do overlay.".into());
                        }
                    } else {
                        let registered = roots.iter().any(|directory| {
                            read(&Path::new(directory).join("installation.json"))
                                .ok()
                                .and_then(|bytes| {
                                    serde_json::from_slice::<InstallationRecord>(&bytes).ok()
                                })
                                .is_some_and(|r| r.prefix == profile.base)
                        });
                        if !registered {
                            return Err(
                                "Inclua a instalação base do overlay na busca e na seleção.".into(),
                            );
                        }
                    }
                    if super::super::game_execution::prefix_in_use(Path::new(&profile.base)) {
                        return Err("Feche as ferramentas Wine da base original antes de importar os overlays.".into());
                    }
                    if locked.insert(profile.base.clone()) {
                        leases.push(super::super::game_installation::lock_prefix(Path::new(
                            &profile.base,
                        ))?);
                    }
                }
            }
        }
        games += record.entries.len();
        writes.push((
            manifest,
            f.original.clone(),
            serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?,
        ));
        if let Some(original) = &f.copy {
            let path = f.directory.join("copy.json");
            if read(&path)? != *original {
                return Err("O registro da cópia mudou. Procure novamente.".into());
            }
            let mut v: serde_json::Value =
                serde_json::from_slice(original).map_err(|_| "Registro da cópia inválido.")?;
            if let Some(source) = v["source"].as_str() {
                v["source"] = serde_json::json!(translate(source, &plan.map));
            }
            writes.push((
                path,
                original.clone(),
                serde_json::to_vec_pretty(&v).map_err(|e| e.to_string())?,
            ));
        }
        roots.insert(f.directory.to_string_lossy().into_owned());
    }
    let mut links = Vec::<(PathBuf, PathBuf, PathBuf)>::new();
    let mut seen_links = BTreeSet::new();
    for f in &plan.found {
        if !chosen.contains(&f.directory.to_string_lossy().into_owned()) {
            continue;
        }
        let record = relocated(&f.record, &plan.map)?;
        for entry in &record.entries {
            let physical = entry
                .profile
                .as_ref()
                .filter(|p| p.kind == super::super::game_profiles::ProfileKind::Overlay)
                .map(|p| Path::new(&p.root).join("upper"))
                .unwrap_or_else(|| PathBuf::from(&entry.prefix));
            let link = physical.join("dosdevices/c:");
            if !seen_links.insert(link.clone()) {
                continue;
            }
            if let Ok(old) = fs::read_link(&link) {
                if old.is_absolute() {
                    let new = PathBuf::from(translate(&old.to_string_lossy(), &plan.map));
                    if new != old {
                        links.push((link, old, new));
                    }
                }
            }
        }
    }
    let replace_link = |path: &Path, target: &Path| -> Result<(), String> {
        let temporary =
            path.with_file_name(format!(".peligames-recovery-{}", uuid::Uuid::new_v4()));
        std::os::unix::fs::symlink(target, &temporary).map_err(|e| e.to_string())?;
        if let Err(e) = fs::rename(&temporary, path) {
            let _ = fs::remove_file(temporary);
            return Err(e.to_string());
        }
        Ok(())
    };
    let mut completed_links = 0;
    let mut completed = 0;
    let result = (|| {
        for (path, _, bytes) in &writes {
            write_json(
                path,
                &serde_json::from_slice::<serde_json::Value>(bytes).map_err(|e| e.to_string())?,
            )?;
            completed += 1;
        }
        for (path, old, new) in &links {
            if fs::read_link(path).ok().as_ref() != Some(old) {
                return Err("O mapeamento do prefixo mudou durante a importação.".into());
            }
            replace_link(path, new)?;
            completed_links += 1;
        }
        write_json(index, &roots)
    })();
    if let Err(error) = result {
        let mut rollback = Vec::new();
        for (path, old, _) in links[..completed_links].iter().rev() {
            if let Err(e) = replace_link(path, old) {
                rollback.push(e);
            }
        }
        for (path, bytes, _) in writes[..completed].iter().rev() {
            if let Err(e) = write_json(
                path,
                &serde_json::from_slice::<serde_json::Value>(bytes).map_err(|e| e.to_string())?,
            ) {
                rollback.push(e);
            }
        }
        if let Err(e) = write_json(index, &before_roots) {
            rollback.push(e);
        }
        return Err(if rollback.is_empty() {
            error
        } else {
            format!(
                "{error}. Falha ao restaurar registros: {}",
                rollback.join("; ")
            )
        });
    }
    let installations = chosen.len();
    *cache = None;
    Ok(Imported {
        installations,
        games,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    static TEST_LOCK: Mutex<()> = Mutex::new(());
    struct Fixture {
        root: PathBuf,
        index: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("peligames-recovery-{}", uuid::Uuid::new_v4()));
            fs::create_dir(&root).unwrap();
            Self {
                index: root.join("index.json"),
                root,
            }
        }
        fn game(&self, name: &str, kind: &str, old_root: &str) -> PathBuf {
            let directory = self.root.join(name);
            fs::create_dir_all(&directory).unwrap();
            let old = Path::new(old_root).join(name);
            let old_prefix = if kind == "overlay" {
                old.join("profiles/instance/prefix")
            } else {
                old.join("prefix")
            };
            let prefix = if kind == "overlay" {
                directory.join("profiles/instance/prefix")
            } else {
                directory.join("prefix")
            };
            fs::create_dir_all(&prefix).unwrap();
            let profile = if kind == "overlay" {
                for folder in ["upper", "work"] {
                    fs::create_dir_all(directory.join("profiles/instance").join(folder)).unwrap();
                }
                serde_json::json!({"kind":"overlay","root":old.join("profiles/instance"),"base":Path::new(old_root).join("Base/prefix")})
            } else {
                fs::create_dir_all(prefix.join("drive_c/Game")).unwrap();
                fs::write(prefix.join("drive_c/Game/game.exe"), b"fixture").unwrap();
                fs::create_dir_all(prefix.join("dosdevices")).unwrap();
                std::os::unix::fs::symlink(
                    old_prefix.join("drive_c"),
                    prefix.join("dosdevices/c:"),
                )
                .unwrap();
                serde_json::Value::Null
            };
            let advanced = serde_json::json!({"schema_version":2,"launch_options":"OLD=1 %command%","wrappers":[{"program":"/old/runtime/wrapper","arguments":"","enabled":true}]});
            let entry = serde_json::json!({"name":name,"path":format!("peligames:{name}"),"directory":old_prefix.join("drive_c/Game"),"executable":old_prefix.join("drive_c/Game/game.exe"),"prefix":old_prefix,"profile":profile,"proton":"/missing/proton","cover_url":null,"launcher":"PeliGames","advanced":advanced});
            let record = serde_json::json!({"name":name,"directory":old,"prefix":old_prefix,"proton":"/missing/proton","installer":"","cover_url":null,"entries":[entry],"selected_executable":old_prefix.join("drive_c/Game/game.exe")});
            fs::write(
                directory.join("installation.json"),
                serde_json::to_vec_pretty(&record).unwrap(),
            )
            .unwrap();
            if kind != "base" {
                fs::write(directory.join("copy.json"),serde_json::to_vec(&serde_json::json!({"version":1,"source":Path::new(old_root).join("Base/prefix"),"kind":kind})).unwrap()).unwrap();
            }
            directory
        }
        fn search(&self) -> Discovery {
            discover_at(&self.index, vec![self.root.to_string_lossy().into_owned()]).unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn moved_library_recovers_base_reflink_and_overlay_without_mounting_or_running() {
        let _guard = TEST_LOCK.lock().unwrap();
        let f = Fixture::new();
        let base = f.game("Base", "base", "/previous/library");
        let overlay = f.game("Overlay", "overlay", "/previous/library");
        let reflink = f.game("Reflink", "reflink", "/previous/library");
        let discovery = f.search();
        assert_eq!(discovery.candidates.len(), 3);
        assert!(discovery.candidates.iter().all(|c| c.error.is_none()));
        let selected = discovery
            .candidates
            .iter()
            .map(|c| c.directory.clone())
            .collect();
        let imported = restore_at(&f.index, discovery.token, selected).unwrap();
        assert_eq!(imported.installations, 3);
        assert_eq!(imported.games, 3);
        let r: InstallationRecord =
            serde_json::from_slice(&fs::read(overlay.join("installation.json")).unwrap()).unwrap();
        let p = r.entries[0].profile.as_ref().unwrap();
        assert_eq!(p.base, base.join("prefix").to_string_lossy());
        assert_eq!(p.root, overlay.join("profiles/instance").to_string_lossy());
        assert!(r.entries[0].advanced.wrappers.is_empty());
        assert!(r.entries[0].advanced.launch_options.is_none());
        assert!(fs::read_dir(overlay.join("profiles/instance/prefix"))
            .unwrap()
            .next()
            .is_none());
        assert_eq!(
            fs::read_link(reflink.join("prefix/dosdevices/c:")).unwrap(),
            reflink.join("prefix/drive_c")
        );
        assert_eq!(
            fs::read(reflink.join("prefix/drive_c/Game/game.exe")).unwrap(),
            b"fixture"
        );
        let v: serde_json::Value =
            serde_json::from_slice(&fs::read(reflink.join("copy.json")).unwrap()).unwrap();
        assert_eq!(v["source"], base.join("prefix").to_string_lossy().as_ref());
        assert!(f.search().candidates.iter().all(|c| c.registered));
    }
    #[test]
    fn missing_base_blocks_overlay_but_reflink_is_independent() {
        let _guard = TEST_LOCK.lock().unwrap();
        let f = Fixture::new();
        f.game("Overlay", "overlay", "/old");
        f.game("Reflink", "reflink", "/old");
        let d = f.search();
        assert!(d
            .candidates
            .iter()
            .find(|c| c.kind == "Overlay")
            .unwrap()
            .error
            .as_ref()
            .unwrap()
            .contains("Base"));
        let copy = d.candidates.iter().find(|c| c.kind == "Reflink").unwrap();
        assert!(copy.error.is_none());
        assert!(restore_at(&f.index, d.token, vec![copy.directory.clone()]).is_ok());
    }
    #[test]
    fn restoring_an_overlay_requires_importing_its_base_and_changed_records_are_rejected() {
        let _guard = TEST_LOCK.lock().unwrap();
        let f = Fixture::new();
        let base = f.game("Base", "base", "/old");
        let overlay = f.game("Overlay", "overlay", "/old");
        let d = f.search();
        assert!(restore_at(
            &f.index,
            d.token,
            vec![overlay.to_string_lossy().into_owned()]
        )
        .unwrap_err()
        .contains("base"));
        assert!(!f.index.exists());
        let d = f.search();
        let original = fs::read(base.join("installation.json")).unwrap();
        fs::write(base.join("installation.json"), b"{}").unwrap();
        assert!(restore_at(&f.index, d.token, vec![base.to_string_lossy().into_owned()]).is_err());
        assert_eq!(fs::read(base.join("installation.json")).unwrap(), b"{}");
        fs::write(base.join("installation.json"), original).unwrap();
    }
    #[test]
    fn discovery_is_read_only_does_not_follow_links_and_limits_invalid_records() {
        let _guard = TEST_LOCK.lock().unwrap();
        let f = Fixture::new();
        let base = f.game("Base", "base", "/old");
        let original = fs::read(base.join("installation.json")).unwrap();
        std::os::unix::fs::symlink(&base, f.root.join("Link")).unwrap();
        let invalid = f.root.join("Bad");
        fs::create_dir(&invalid).unwrap();
        fs::write(invalid.join("installation.json"), b"not json").unwrap();
        let d = f.search();
        assert_eq!(d.candidates.len(), 1);
        assert!(!d.warnings.is_empty());
        assert_eq!(fs::read(base.join("installation.json")).unwrap(), original);
        assert!(!f.index.exists());
    }
    #[test]
    fn separate_disks_and_prefix_at_installation_root_are_recognized() {
        let _guard = TEST_LOCK.lock().unwrap();
        let base_disk = Fixture::new();
        let copy_disk = Fixture::new();
        let base = base_disk.game("Base", "base", "/previous");
        let overlay = copy_disk.game("Overlay", "overlay", "/previous");
        let record = base.join("installation.json");
        let raw = fs::read_to_string(&record)
            .unwrap()
            .replace("/previous/Base/prefix", "/previous/Base");
        fs::write(&record, raw).unwrap();
        fs::rename(base.join("prefix/drive_c"), base.join("drive_c")).unwrap();
        fs::rename(base.join("prefix/dosdevices"), base.join("dosdevices")).unwrap();
        fs::remove_dir(base.join("prefix")).unwrap();
        let overlay_record = overlay.join("installation.json");
        let raw = fs::read_to_string(&overlay_record)
            .unwrap()
            .replace("/previous/Base/prefix", "/previous/Base");
        fs::write(&overlay_record, raw).unwrap();
        let d = discover_at(
            &base_disk.index,
            vec![
                base_disk.root.to_string_lossy().into_owned(),
                copy_disk.root.to_string_lossy().into_owned(),
            ],
        )
        .unwrap();
        assert_eq!(d.candidates.len(), 2);
        assert!(d.candidates.iter().all(|c| c.error.is_none()));
        let selected = d.candidates.iter().map(|c| c.directory.clone()).collect();
        assert_eq!(
            restore_at(&base_disk.index, d.token, selected)
                .unwrap()
                .installations,
            2
        );
        let saved: InstallationRecord =
            serde_json::from_slice(&fs::read(overlay_record).unwrap()).unwrap();
        assert_eq!(
            saved.entries[0].profile.as_ref().unwrap().base,
            base.to_string_lossy()
        );
    }
}
