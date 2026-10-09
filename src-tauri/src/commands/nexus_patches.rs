//! Helldivers patches have per-archive contiguous numbering, not ordinary ZIP paths.
use super::{nexus_local::NexusGame, nexus_modules::Definition};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};
pub(super) fn parts(name: &str) -> Option<(String, u32, String)> {
    let (hash, rest) = name.split_once(".patch_")?;
    if hash.len() != 16 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let (index, suffix) = rest
        .split_once('.')
        .map(|(i, s)| (i, format!(".{s}")))
        .unwrap_or((rest, String::new()));
    if !index.bytes().all(|b| b.is_ascii_digit())
        || index.is_empty()
        || !["", ".stream", ".gpu_resources"].contains(&suffix.as_str())
    {
        return None;
    }
    let index = index.parse::<u32>().ok()?;
    if index > 10000 {
        return None;
    }
    Some((hash.to_ascii_lowercase(), index, suffix))
}
pub(super) struct Planner {
    root: PathBuf,
    next: BTreeMap<String, u32>,
    active: bool,
}
impl Planner {
    pub(super) fn new(game: &NexusGame, module: &Definition) -> Result<Self, String> {
        let root = PathBuf::from(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
        let active = module.domain == "helldivers2";
        let mut next = BTreeMap::new();
        if active {
            let manifest = super::nexus_profile::directory(game)?.join("deployment.json");
            let old: serde_json::Value = match fs::read(manifest) {
                Ok(b) => serde_json::from_slice(&b).map_err(|e| e.to_string())?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => serde_json::Value::Null,
                Err(e) => return Err(e.to_string()),
            };
            let mut external: BTreeMap<String, BTreeSet<u32>> = BTreeMap::new();
            if let Ok(entries) = fs::read_dir(root.join("data")) {
                for entry in entries.take(10000) {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some((hash, index, suffix)) = parts(&name) {
                        if suffix.is_empty() && !old["files"].get(format!("data/{name}")).is_some()
                        {
                            external.entry(hash).or_default().insert(index);
                        }
                    }
                }
            }
            for (hash, indices) in external {
                if indices.iter().copied().ne(0..indices.len() as u32) {
                    return Err(format!("Patches externos de {hash} têm lacunas; preserve-os e corrija a ordem antes de instalar."));
                }
                next.insert(hash, indices.len() as u32);
            }
        }
        Ok(Self { root, next, active })
    }
    pub(super) fn map(&mut self, entries: &[String]) -> Result<BTreeMap<String, String>, String> {
        if !self.active {
            return Ok(BTreeMap::new());
        }
        if entries.iter().any(|e| {
            e.rsplit('/')
                .next()
                .is_some_and(|n| n.eq_ignore_ascii_case("mod_manifest.json"))
        }) {
            return Err("Este pacote Helldivers oferece escolhas em mod_manifest.json. Importe somente a alternativa escolhida; nenhuma escolha é aplicada automaticamente.".into());
        }
        let mut groups: BTreeMap<(String, u32), Vec<(String, String)>> = BTreeMap::new();
        for source in entries {
            if let Some((hash, index, suffix)) = parts(source.rsplit('/').next().unwrap_or(source))
            {
                groups
                    .entry((hash, index))
                    .or_default()
                    .push((source.clone(), suffix));
            }
        }
        let mut result = BTreeMap::new();
        for ((hash, _), files) in groups {
            if !self.root.join("data").join(&hash).is_file() {
                return Err(format!(
                    "O jogo não contém o arquivo base {hash}; este patch é de outra versão."
                ));
            }
            let mut suffixes = BTreeSet::new();
            for (_, suffix) in &files {
                if !suffixes.insert(suffix) {
                    return Err(
                        "O pacote contém alternativas do mesmo patch. Importe só a opção desejada."
                            .into(),
                    );
                }
            }
            if !suffixes.contains(&String::new()) {
                return Err(
                    "Patch .stream/.gpu_resources sem o arquivo .patch_N principal.".into(),
                );
            }
            let next = self.next.entry(hash.clone()).or_default();
            let index = *next;
            *next += 1;
            for (source, suffix) in files {
                result.insert(source, format!("data/{hash}.patch_{index}{suffix}"));
            }
        }
        Ok(result)
    }
}
