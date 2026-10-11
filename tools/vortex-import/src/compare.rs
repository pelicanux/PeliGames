//! Comparação entre os fatos extraídos do Vortex e os módulos Nexus do PeliGames.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::extract::GameFacts;

#[derive(Debug, Clone)]
pub struct PeliModule {
    pub file: String,
    pub domain: String,
    pub name: String,
    pub steam_id: Option<String>,
    pub executable: Option<String>,
    pub required: Vec<String>,
    pub destinations: BTreeSet<String>,
}

pub fn load_modules(dir: &Path) -> Result<Vec<PeliModule>, String> {
    let mut modules = Vec::new();
    let mut entries: Vec<_> = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let json: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let file = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        let items: Vec<&Value> = match &json {
            Value::Array(items) => items.iter().collect(),
            Value::Object(o) if o.contains_key("domain") && o.contains_key("routes") => vec![&json],
            _ => continue,
        };
        for item in items {
            if let Some(m) = module_from_json(&file, item) {
                modules.push(m);
            }
        }
    }
    Ok(modules)
}

fn module_from_json(file: &str, v: &Value) -> Option<PeliModule> {
    let domain = v.get("domain")?.as_str()?.to_string();
    let strings = |key: &str| -> Vec<String> {
        v.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
    };
    let mut destinations: BTreeSet<String> = strings("destinations").into_iter().map(|d| norm_dest(&d)).collect();
    if let Some(routes) = v.get("routes").and_then(Value::as_array) {
        for r in routes {
            if let Some(t) = r.get("target").and_then(Value::as_str) {
                destinations.insert(norm_dest(t));
            }
        }
    }
    // Mesma expansão da família Unreal feita pelo launcher (nexus_modules.rs).
    if let Some(family) = v.get("family").filter(|f| f.get("kind").and_then(Value::as_str) == Some("unreal")) {
        let project = family.get("project").and_then(Value::as_str).unwrap_or_default();
        let binaries = family.get("binaries").and_then(Value::as_str).unwrap_or("Win64");
        let ue4ss = family.get("ue4ss").and_then(Value::as_bool).unwrap_or(true);
        destinations.insert(norm_dest(&format!("{project}/Content/Paks/~mods")));
        if ue4ss {
            for d in ["Content/Paks/LogicMods".to_string(), format!("Binaries/{binaries}/ue4ss"), format!("Binaries/{binaries}/ue4ss/Mods"), format!("Binaries/{binaries}/dwmapi.dll")] {
                destinations.insert(norm_dest(&format!("{project}/{d}")));
            }
        }
    }
    destinations.retain(|d| !d.is_empty());
    Some(PeliModule {
        file: file.to_string(),
        name: v.get("name").and_then(Value::as_str).unwrap_or(&domain).to_string(),
        domain,
        steam_id: v.get("steam_id").and_then(|s| s.as_str().map(str::to_string).or_else(|| s.as_u64().map(|n| n.to_string()))),
        executable: v.get("executable").and_then(Value::as_str).map(str::to_string),
        required: strings("required"),
        destinations,
    })
}

/// Caminho relativo à raiz do jogo, sem marcadores variáveis no fim.
fn norm_dest(p: &str) -> String {
    let p = p.replace('\\', "/");
    let p = p.strip_prefix("{gamePath}").unwrap_or(&p);
    let mut out = Vec::new();
    for seg in p.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        }
        if seg.contains('{') {
            break;
        }
        out.push(seg.to_lowercase());
    }
    out.join("/")
}

fn vortex_destinations(f: &GameFacts) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    let mut add = |t: &str| {
        // Destinos fora da pasta do jogo mantêm o marcador para comparação.
        if t.starts_with("{gamePath}") || !t.starts_with('{') {
            let d = norm_dest(t);
            set.insert(if d.is_empty() { "(raiz)".to_string() } else { d });
        } else {
            set.insert(t.to_lowercase());
        }
    };
    for mt in &f.mod_types {
        if let Some(t) = &mt.target {
            if !t.contains("{?") {
                add(t);
            }
        }
    }
    if let Some(mp) = &f.mod_path {
        if !mp.contains("{?") {
            add(mp);
        }
    }
    set
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub domain: String,
    pub name: String,
    pub module_file: String,
    pub vortex: Option<String>,
    pub vortex_level: Option<String>,
    pub matched_by: Option<String>,
    pub steam: Check,
    pub executable: Check,
    pub required_overlap: Vec<String>,
    pub shared_destinations: Vec<String>,
    pub peligames_only: Vec<String>,
    pub vortex_only: Vec<String>,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Igual,
    Diferente,
    SemDado,
}

impl Check {
    pub fn icon(self) -> &'static str {
        match self {
            Check::Igual => "✅",
            Check::Diferente => "⚠️",
            Check::SemDado => "—",
        }
    }
}

fn find_match<'f>(m: &PeliModule, facts: &'f [GameFacts]) -> Option<(&'f GameFacts, &'static str)> {
    let domain = m.domain.to_lowercase();
    let has_steam = |f: &GameFacts| m.steam_id.as_ref().is_some_and(|s| f.stores.steam.contains(s) || f.stores.unclassified.contains(s));
    // Edições diferentes (ex.: Skyrim SE e VR) compartilham o domínio Nexus:
    // o Steam ID desempata antes da riqueza dos dados.
    let by = |pred: &dyn Fn(&GameFacts) -> bool| {
        facts.iter().filter(|f| pred(f)).max_by_key(|f| (has_steam(f), f.stub_repository.is_none(), f.mod_types.len() + f.installers.len()))
    };
    if let Some(f) = by(&|f| f.game_id.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(&domain))) {
        return Some((f, "ID do jogo"));
    }
    if let Some(f) = by(&|f| f.nexus_domain.as_deref().is_some_and(|d| d.eq_ignore_ascii_case(&domain))) {
        return Some((f, "domínio Nexus"));
    }
    if let Some(steam) = &m.steam_id {
        if let Some(f) = by(&|f| f.stores.steam.contains(steam) || f.stores.unclassified.contains(steam)) {
            return Some((f, "Steam ID"));
        }
    }
    None
}

pub fn compare(modules: &[PeliModule], facts: &[GameFacts]) -> Vec<Comparison> {
    modules
        .iter()
        .map(|m| {
            let found = find_match(m, facts);
            let Some((f, by)) = found else {
                return Comparison {
                    domain: m.domain.clone(),
                    name: m.name.clone(),
                    module_file: m.file.clone(),
                    vortex: None,
                    vortex_level: None,
                    matched_by: None,
                    steam: Check::SemDado,
                    executable: Check::SemDado,
                    required_overlap: Vec::new(),
                    shared_destinations: Vec::new(),
                    peligames_only: m.destinations.iter().cloned().collect(),
                    vortex_only: Vec::new(),
                };
            };
            let steam = match &m.steam_id {
                Some(s) if !f.stores.steam.is_empty() || !f.stores.unclassified.is_empty() => {
                    if f.stores.steam.contains(s) || f.stores.unclassified.contains(s) { Check::Igual } else { Check::Diferente }
                }
                _ => Check::SemDado,
            };
            let base = |p: &str| p.replace('\\', "/").rsplit('/').next().unwrap_or(p).to_lowercase();
            let executable = match (&m.executable, &f.executable) {
                (Some(a), Some(b)) => {
                    if base(a) == base(b) || f.required_files.iter().any(|r| base(r) == base(a)) { Check::Igual } else { Check::Diferente }
                }
                _ => Check::SemDado,
            };
            let vortex_required: BTreeSet<String> = f.required_files.iter().map(|r| r.to_lowercase()).collect();
            let required_overlap = m.required.iter().filter(|r| vortex_required.contains(&r.to_lowercase())).cloned().collect();

            let vortex = vortex_destinations(f);
            let related = |a: &str, b: &str| a == b || a.starts_with(&format!("{b}/")) || b.starts_with(&format!("{a}/"));
            let ours: BTreeSet<String> = m.destinations.iter().map(|d| if d.is_empty() { "(raiz)".into() } else { d.clone() }).collect();
            let shared: Vec<String> = ours.iter().filter(|o| vortex.iter().any(|v| related(o, v))).cloned().collect();
            let peligames_only = ours.iter().filter(|o| !vortex.iter().any(|v| related(o, v))).cloned().collect();
            let vortex_only = vortex.iter().filter(|v| !ours.iter().any(|o| related(o, v))).cloned().collect();
            Comparison {
                domain: m.domain.clone(),
                name: m.name.clone(),
                module_file: m.file.clone(),
                vortex: Some(f.key()),
                vortex_level: Some(format!("{:?}", f.classification.level).to_lowercase()),
                matched_by: Some(by.to_string()),
                steam,
                executable,
                required_overlap,
                shared_destinations: shared,
                peligames_only,
                vortex_only,
            }
        })
        .collect()
}
