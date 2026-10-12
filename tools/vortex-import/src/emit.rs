//! Geração de módulos das famílias Unreal e Unity a partir dos fatos do Vortex.
//!
//! Só emite jogos cujos dados de identificação foram todos resolvidos; o
//! comportamento de instalação vem do perfil da família no launcher.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::extract::GameFacts;

/// Lista pública de jogos do Nexus e IDs com extensão Vortex publicada.
#[derive(Default)]
pub struct NexusIndex {
    pub games: BTreeMap<String, u64>,
    pub with_extension: BTreeSet<u64>,
}

impl NexusIndex {
    pub fn load(games: &std::path::Path, manifest: &std::path::Path) -> Result<Self, String> {
        let read = |p: &std::path::Path| -> Result<Value, String> {
            serde_json::from_str(&std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?).map_err(|e| format!("{}: {e}", p.display()))
        };
        let games = read(games)?
            .as_array()
            .ok_or("lista de jogos inválida")?
            .iter()
            .filter_map(|g| Some((g["domain_name"].as_str()?.to_string(), g["id"].as_u64()?)))
            .collect();
        let with_extension = read(manifest)?
            .pointer("/data/extensions")
            .and_then(Value::as_array)
            .ok_or("manifesto de extensões inválido")?
            .iter()
            .filter(|e| e["type"] == "game")
            .filter_map(|e| e["game_id"].as_str().and_then(|id| id.parse().ok()).or_else(|| e["game_id"].as_u64()))
            .collect();
        Ok(Self { games, with_extension })
    }
}

pub struct Skipped {
    pub game: String,
    pub reason: String,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum EmitFamily {
    Unreal,
    Unity,
    ReEngine,
}

impl EmitFamily {
    pub const ALL: [EmitFamily; 3] = [EmitFamily::Unreal, EmitFamily::Unity, EmitFamily::ReEngine];

    pub fn name(self) -> &'static str {
        match self {
            EmitFamily::Unreal => "unreal",
            EmitFamily::Unity => "unity",
            EmitFamily::ReEngine => "re_engine",
        }
    }

    fn matches(self, f: &GameFacts) -> bool {
        match self {
            EmitFamily::Unreal => is_unreal(f),
            EmitFamily::Unity => is_unity(f),
            EmitFamily::ReEngine => is_re_engine(f),
        }
    }

    fn module(self, f: &GameFacts, evidence: &str, nexus: Option<&NexusIndex>) -> Result<(Value, Value), String> {
        match self {
            EmitFamily::Unreal => unreal_module(f, evidence, nexus),
            EmitFamily::Unity => unity_module(f, evidence, nexus),
            EmitFamily::ReEngine => re_engine_module(f, evidence, nexus),
        }
    }
}

/// Template "Fluffy + REFramework": só jogos cuja extensão instala o REFramework.
fn is_re_engine(f: &GameFacts) -> bool {
    let family = f.family.as_deref().unwrap_or_default();
    (family.contains("RE Engine") || family.contains("Fluffy")) && f.mod_types.iter().any(|m| m.id.ends_with("-reframework"))
}

pub fn re_engine_module(f: &GameFacts, evidence: &str, nexus: Option<&NexusIndex>) -> Result<(Value, Value), String> {
    let (domain, name, steam, nexus_game_id) = identity_facts(f, nexus)?;
    // A constante EXEC é o jogo completo; o primeiro retorno pode ser a demo.
    let exec = f.constants.get("EXEC").filter(|e| !e.contains("XXX")).cloned().or_else(|| f.executable.clone()).ok_or("executável não resolvido")?;
    let lower = exec.to_ascii_lowercase();
    if !relative_file(&exec) || !lower.ends_with(".exe") || lower == "gamelaunchhelper.exe" || lower.contains("trial") || lower.contains("demo") {
        return Err(format!("executável inválido: {exec}"));
    }
    let mut module = json!({
        "schema": 1,
        "version": "1.0.0",
        "domain": domain,
        "name": name,
        "platform": "proton",
        "engine": "deployment",
        "family": { "kind": "re_engine" },
        "executable": exec,
        "required": [exec],
        "header": [77, 90],
        "destinations": [],
        "routes": [],
        "sources": [evidence],
        "credits": [CREDITS],
        "notes": ["Módulo gerado do template RE Engine do Vortex: scripts, plugins e dados REFramework e o carregador dinput8.dll. Pacotes natives/, pak_mods e mods Fluffy são recusados. Estrutura verificada por testes sintéticos; não validado no jogo."],
    });
    if let Some(id) = &steam {
        module["steam_id"] = json!(id);
    }
    // REFramework: the game's Nexus page, or the nightly GitHub build (`useRefNightly`).
    if f.constants.get("useRefNightly").is_some_and(|v| v == "true") {
        module["downloads"] = json!([{ "name": "REFramework", "repository": "praydog/REFramework-nightly", "prefix": "REFramework", "present": ["dinput8.dll"] }]);
    } else if let Some((id, domain)) = nexus_page(f, "REF_PAGE_NO", "REF_DOMAIN") {
        module["downloads"] = json!([{ "name": "REFramework", "source": "nexus", "mod_id": id, "domain": domain, "prefix": "REFramework", "present": ["dinput8.dll"] }]);
    }
    let cases = vec![
        json!({ "source": "dinput8.dll", "target": "dinput8.dll" }),
        json!({ "source": "Mod/reframework/autorun/example.lua", "target": "reframework/autorun/example.lua" }),
        json!({ "source": "reframework/plugins/example.dll", "target": "reframework/plugins/example.dll" }),
        json!({ "source": "Pack/reframework/data/example.json", "target": "reframework/data/example.json" }),
    ];
    let audit = audit_entry(&domain, &name, evidence, cases,
        "Identificação (executável, Steam ID) extraída da extensão; rotas REFramework do perfil RE Engine compartilhado. Mods Fluffy, natives e presets da extensão não são aplicados.",
        nexus_game_id);
    Ok((module, audit))
}

/// Nexus page of a template requirement: page number and its game (`site` or the module's own).
fn nexus_page(f: &GameFacts, page: &str, domain: &str) -> Option<(u64, Option<String>)> {
    let id = f.constants.get(page)?.parse::<u64>().ok().filter(|id| *id > 0)?;
    let site = f.constants.get(domain).is_some_and(|d| d == "site");
    Some((id, site.then(|| "site".to_string())))
}

/// Somente o template híbrido BepInEx/MelonLoader; UMM e loaders próprios ficam fora.
fn is_unity(f: &GameFacts) -> bool {
    let family = f.family.as_deref().unwrap_or_default().to_ascii_lowercase();
    family.starts_with("unity bepinex/melonloader") && !family.contains("mscloader")
}

/// Identificação comum: domínio, nome, Steam ID e ID do jogo no Nexus.
fn identity_facts(f: &GameFacts, nexus: Option<&NexusIndex>) -> Result<(String, String, Option<String>, Option<u64>), String> {
    let domain = f.nexus_domain.clone().or_else(|| f.game_id.clone()).ok_or("sem ID")?;
    if !domain.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()) {
        return Err(format!("domínio fora do padrão: {domain}"));
    }
    let name = f.name.clone().ok_or("sem nome")?;
    if name.contains("XXX") {
        return Err("template sem preencher (XXX)".into());
    }
    let nexus_game_id = match nexus {
        Some(index) => {
            let id = *index.games.get(&domain).ok_or_else(|| format!("domínio inexistente no Nexus: {domain}"))?;
            if !index.with_extension.contains(&id) {
                return Err("sem extensão Vortex publicada no manifesto público".into());
            }
            Some(id)
        }
        None => None,
    };
    let steam = f
        .constants
        .get("STEAMAPP_ID")
        .filter(|id| f.stores.steam.contains(*id))
        .cloned()
        .or_else(|| f.stores.steam.iter().next().cloned())
        .filter(|id| id.bytes().all(|b| b.is_ascii_digit()));
    Ok((domain, name, steam, nexus_game_id))
}

fn audit_entry(domain: &str, name: &str, evidence: &str, cases: Vec<Value>, finding: &str, nexus_game_id: Option<u64>) -> Value {
    let mut audit = json!({
        "domain": domain,
        "name": name,
        "module_version": "1.0.0",
        "vortex_status": "confirmed",
        "vortex_evidence": [evidence],
        "extension_code_compared": false,
        "generated_by": "tools/vortex-import",
        "framework_ids_checked": [],
        "route_sample_kind": "synthetic_family_layout",
        "route_cases": cases,
        "findings": [finding],
        "pending_checks": [
            "Conferir domínio Nexus e executáveis em uma instalação real.",
            "Amostragem de pacotes reais populares e recentes.",
            "Execução no jogo real com versão do jogo, loader e Proton registradas."
        ],
        "runtime_verified": false
    });
    if let Some(id) = nexus_game_id {
        audit["nexus_game_id"] = json!(id);
    }
    audit
}

/// Vortex mod type id → path relative to the game root (root and unresolved
/// targets left out). The launcher only uses entries inside module destinations.
fn vortex_mod_types(f: &GameFacts) -> serde_json::Map<String, Value> {
    f.mod_types.iter().filter_map(|m| {
        let rel = m.target.as_deref()?.strip_prefix("{gamePath}/")?;
        (!rel.is_empty() && !rel.contains('{') && !rel.split('/').any(|p| p.is_empty() || p == "..")).then(|| (m.id.clone(), json!(rel)))
    }).collect()
}

const CREDITS: &str = "Fatos extraídos por análise estática da extensão Vortex (GPL-3.0) com tools/vortex-import; implementação independente PeliGames.";

pub fn unity_module(f: &GameFacts, evidence: &str, nexus: Option<&NexusIndex>) -> Result<(Value, Value), String> {
    let (domain, name, steam, nexus_game_id) = identity_facts(f, nexus)?;
    let constant = |key: &str| f.constants.get(key).filter(|v| !v.is_empty() && !v.contains("XXX")).cloned();
    if constant("isXna").as_deref() == Some("true") {
        return Err("jogo XNA: sem pasta de dados Unity".into());
    }
    if constant("ARCH").is_some_and(|a| a != "x64") {
        return Err("arquitetura diferente de x64".into());
    }
    let data = constant("DATA_FOLDER_DEFAULT").ok_or("pasta _Data não resolvida")?;
    if !data.ends_with("_Data") || data.contains(['/', '\\', '{']) {
        return Err(format!("pasta de dados inválida: {data}"));
    }
    let exec = constant("EXEC").or_else(|| f.executable.clone()).ok_or("executável não resolvido")?;
    if !relative_file(&exec) || !exec.to_ascii_lowercase().ends_with(".exe") || exec.eq_ignore_ascii_case("gamelaunchhelper.exe") {
        return Err(format!("executável inválido: {exec}"));
    }
    // Sem escolha de carregador, a extensão instala só o recomendado.
    let choice = constant("loaderChoice").as_deref() != Some("false");
    let loaders: Vec<&str> = match (choice, constant("recommendedLoader").as_deref()) {
        (true, _) => vec!["bepinex", "melonloader"],
        (false, Some("bep")) => vec!["bepinex"],
        (false, Some("mel")) => vec!["melonloader"],
        (false, other) => return Err(format!("carregador recomendado desconhecido: {other:?}")),
    };
    let mut module = json!({
        "schema": 1,
        "version": "1.0.0",
        "domain": domain,
        "name": name,
        "platform": "proton",
        "engine": "deployment",
        "family": { "kind": "unity", "data": data, "loaders": loaders },
        "executable": exec,
        "required": [exec, "UnityPlayer.dll"],
        "header": [77, 90],
        "destinations": [],
        "routes": [],
        "sources": [evidence],
        "credits": [CREDITS],
        "notes": [format!(
            "Módulo gerado do template Unity do Vortex ({}): loader, plugins e mods conforme a DLL referenciada. Substituições em {data}, Assembly e .assets não são aplicadas. Estrutura verificada por testes sintéticos; não validado no jogo.",
            loaders.iter().map(|l| if *l == "bepinex" { "BepInEx" } else { "MelonLoader" }).collect::<Vec<_>>().join("/")
        )],
    });
    // Loader downloads of the template (GitHub release, BepInEx Bleeding Edge
    // build for IL2CPP, or the game's Nexus page when the extension names one).
    let arch = constant("ARCH").unwrap_or_else(|| "x64".into());
    let il2cpp = constant("BEPINEX_BUILD").as_deref() == Some("il2cpp");
    let page = |key: &str, allow: &str| constant(key).and_then(|v| v.parse::<u64>().ok()).filter(|id| *id > 0 && constant(allow).as_deref() != Some("false"));
    let recommended = constant("recommendedLoader");
    let mut downloads = Vec::new();
    if loaders.contains(&"bepinex") {
        let mut download = if let Some(id) = page("BEPINEX_PAGE_NO", "allowBepinexNexus") {
            json!({ "name": "BepInEx", "source": "nexus", "mod_id": id, "prefix": "BepInEx" })
        } else if il2cpp {
            let build = constant("BEP_BE_VER").ok_or("build Bleeding Edge do BepInEx não resolvida")?;
            let commit = constant("BEP_BE_COMMIT").ok_or("commit Bleeding Edge do BepInEx não resolvido")?;
            let artifact = format!("BepInEx-Unity.IL2CPP-win-{arch}");
            json!({ "name": "BepInEx", "source": "bepinex_be", "prefix": format!("{artifact}-"),
                "fallback": format!("https://builds.bepinex.dev/projects/bepinex_be/{build}/{artifact}-6.0.0-be.{build}%2B{commit}.zip") })
        } else {
            json!({ "name": "BepInEx", "repository": "BepInEx/BepInEx", "prefix": format!("BepInEx_win_{arch}_") })
        };
        download["present"] = json!(["winhttp.dll", if il2cpp { "BepInEx/core/BepInEx.Core.dll" } else { "BepInEx/core/BepInEx.dll" }]);
        download["group"] = json!("loader");
        download["recommended"] = json!(loaders.len() == 1 || recommended.as_deref() == Some("bep"));
        downloads.push(download);
    }
    if loaders.contains(&"melonloader") {
        let mut download = if let Some(id) = page("MELON_PAGE_NO", "allowMelonNexus") {
            json!({ "name": "MelonLoader", "source": "nexus", "mod_id": id, "prefix": "MelonLoader" })
        } else {
            json!({ "name": "MelonLoader", "repository": "LavaGang/MelonLoader", "prefix": format!("MelonLoader.{arch}.") })
        };
        download["present"] = json!(["version.dll", "MelonLoader"]);
        download["group"] = json!("loader");
        download["recommended"] = json!(loaders.len() == 1 || recommended.as_deref() == Some("mel"));
        downloads.push(download);
    }
    if !downloads.is_empty() { module["downloads"] = Value::Array(downloads); }
    if let Some(id) = &steam {
        module["steam_id"] = json!(id);
    }
    let types = vortex_mod_types(f);
    if !types.is_empty() {
        module["vortex_mod_types"] = Value::Object(types);
    }
    if constant("hasCustomLoader").as_deref() == Some("true") {
        module["notes"].as_array_mut().unwrap().push(json!("A extensão Vortex também oferece um carregador próprio do jogo; mods desse carregador não são reconhecidos."));
    }
    let mut cases = Vec::new();
    if loaders.contains(&"bepinex") {
        cases.extend([
            json!({ "source": "BepInExPack/BepInEx/core/BepInEx.dll", "target": "BepInEx/core/BepInEx.dll" }),
            json!({ "source": "BepInExPack/winhttp.dll", "target": "winhttp.dll" }),
            json!({ "source": "Example/plugins/Example/Example.dll", "target": "BepInEx/plugins/Example/Example.dll" }),
            json!({ "source": "Example/config/example.cfg", "target": "BepInEx/config/example.cfg" }),
        ]);
    }
    if loaders.contains(&"melonloader") {
        cases.extend([
            json!({ "source": "MelonLoader/net6/MelonLoader.dll", "target": "MelonLoader/net6/MelonLoader.dll" }),
            json!({ "source": "version.dll", "target": "version.dll" }),
            json!({ "source": "Example/Mods/ExampleMelon.dll", "target": "Mods/ExampleMelon.dll" }),
            json!({ "source": "Example/UserLibs/Shared.dll", "target": "UserLibs/Shared.dll" }),
        ]);
    }
    let audit = audit_entry(&domain, &name, evidence, cases,
        "Identificação (executável, pasta de dados, Steam ID, carregadores) extraída da extensão; rotas do perfil Unity compartilhado. DLLs soltas são classificadas pelo carregador referenciado; mods para o carregador errado, pacotes mistos e substituições de dados do jogo são recusados.",
        nexus_game_id);
    Ok((module, audit))
}

fn is_unreal(f: &GameFacts) -> bool {
    let family = f.family.as_deref().unwrap_or_default().to_ascii_lowercase();
    (family.contains("unreal") || family.starts_with("ue4") || family.starts_with("ue5") || family == "ue4")
        && !family.contains("2/3")
        && !family.contains("2-3")
        && !family.contains("tfc")
}

fn project_of(f: &GameFacts) -> Option<String> {
    let candidates = f.mod_types.iter().filter_map(|m| m.target.clone()).chain(f.mod_path.clone());
    for target in candidates {
        let t = target.strip_prefix("{gamePath}/").unwrap_or(&target);
        if let Some(project) = t.strip_suffix("/Content/Paks/~mods") {
            if !project.is_empty() && !project.contains('{') {
                return Some(project.to_string());
            }
        }
    }
    None
}

fn relative_file(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains(['\\', '{', '}', ':'])
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

/// Módulo e entrada de auditoria para um jogo, ou o motivo da recusa.
pub fn unreal_module(f: &GameFacts, evidence: &str, nexus: Option<&NexusIndex>) -> Result<(Value, Value), String> {
    let (domain, name, steam, nexus_game_id) = identity_facts(f, nexus)?;
    let project = project_of(f).ok_or("pasta ~mods do projeto não resolvida")?;
    if let Some(code) = f.constants.get("EPIC_CODE_NAME") {
        if !code.eq_ignore_ascii_case(&project) {
            return Err(format!("projeto divergente: {code} / {project}"));
        }
    }
    let binaries = f.constants.get("EXEC_FOLDER_DEFAULT").cloned().unwrap_or_else(|| "Win64".into());
    let shipping = f.constants.get("SHIPPING_EXE").cloned().ok_or("executável Shipping não resolvido")?;
    if !relative_file(&shipping) || !shipping.to_ascii_lowercase().ends_with(".exe") || !shipping.starts_with(&format!("{project}/Binaries/")) {
        return Err(format!("executável Shipping fora do projeto: {shipping}"));
    }
    let launcher = f.executable.clone().ok_or("executável não resolvido")?;
    if !relative_file(&launcher) || !launcher.to_ascii_lowercase().ends_with(".exe") {
        return Err(format!("executável inválido: {launcher}"));
    }
    let ue4ss = f.mod_types.iter().any(|m| ["-logicmods", "-ue4ss", "-scripts"].iter().any(|s| m.id.ends_with(s)));

    let mut family = json!({ "kind": "unreal", "project": project });
    if binaries != "Win64" {
        family["binaries"] = json!(binaries);
    }
    if !ue4ss {
        family["ue4ss"] = json!(false);
    }
    let mut required = vec![shipping.clone()];
    if launcher != shipping {
        required.insert(0, launcher.clone());
    }
    let summary = if ue4ss {
        "paks em ~mods, LogicMods, UE4SS e scripts Lua/DLL"
    } else {
        "paks em ~mods (sem rotas UE4SS na extensão)"
    };
    let mut module = json!({
        "schema": 1,
        "version": "1.0.0",
        "domain": domain,
        "name": name,
        "platform": "proton",
        "engine": "deployment",
        "family": family,
        "executable": shipping,
        "required": required,
        "header": [77, 90],
        "destinations": [],
        "routes": [],
        "sources": [evidence],
        "credits": [CREDITS],
        "notes": [format!("Módulo gerado do template Unreal do Vortex: {summary}. Estrutura verificada por testes sintéticos; não validado no jogo.")],
    });
    if launcher != shipping {
        module["launch_executable"] = json!(launcher);
    }
    if let Some(id) = steam {
        module["steam_id"] = json!(id);
    }
    // The extension's UE4SS download: its curated Nexus page when it names one,
    // otherwise the GitHub release; offered by the game checks.
    let automatic = f.constants.get("autoDownloadUe4ss").is_some_and(|v| v == "true");
    let mut downloads = Vec::new();
    if ue4ss {
        if let Some((id, domain)) = nexus_page(f, "UE4SS_PAGE_NO", "UE4SS_DOMAIN") {
            downloads.push(json!({ "name": "UE4SS", "source": "nexus", "mod_id": id, "domain": domain, "prefix": "UE4SS", "automatic": automatic }));
        } else if let Some(download) = f.downloads.iter().find(|d| d.name == "UE4SS") {
            downloads.push(json!({ "name": download.name, "repository": download.repository, "tag": download.tag, "prefix": download.prefix, "automatic": download.automatic }));
        }
    }
    // Universal Sig Bypasser for games with .sig files (`SIGBYPASS_REQUIRED`).
    if f.constants.get("SIGBYPASS_REQUIRED").is_some_and(|v| v == "true") {
        if let Some((id, domain)) = nexus_page(f, "SIGBYPASS_PAGE_NO", "SIGBYPASS_DOMAIN") {
            family["sig_bypass"] = json!(true);
            module["family"] = family.clone();
            downloads.push(json!({ "name": "Sig Bypass", "source": "nexus", "mod_id": id, "domain": domain, "prefix": "dsound", "present": [format!("{project}/Binaries/{binaries}/dsound.dll")] }));
        }
    }
    if !downloads.is_empty() { module["downloads"] = Value::Array(downloads); }
    let types = vortex_mod_types(f);
    if !types.is_empty() {
        module["vortex_mod_types"] = Value::Object(types);
    }

    let bin = format!("{project}/Binaries/{binaries}");
    let mut cases = vec![
        json!({ "source": "Example_P.pak", "target": format!("{project}/Content/Paks/~mods/Example_P.pak") }),
        json!({ "source": "Example_P.utoc", "target": format!("{project}/Content/Paks/~mods/Example_P.utoc") }),
        json!({ "source": "Example_P.ucas", "target": format!("{project}/Content/Paks/~mods/Example_P.ucas") }),
    ];
    if ue4ss {
        cases.extend([
            json!({ "source": "Release/LogicMods/ExampleLogic.pak", "target": format!("{project}/Content/Paks/LogicMods/ExampleLogic.pak") }),
            json!({ "source": "Release/Mods/ExampleScript/Scripts/main.lua", "target": format!("{bin}/ue4ss/Mods/ExampleScript/Scripts/main.lua") }),
            json!({ "source": "Release/Mods/ExampleScript/enabled.txt", "target": format!("{bin}/ue4ss/Mods/ExampleScript/enabled.txt") }),
            json!({ "source": "UE4SS_v3.0.1/ue4ss/UE4SS.dll", "target": format!("{bin}/ue4ss/UE4SS.dll") }),
            json!({ "source": "UE4SS_v3.0.1/dwmapi.dll", "target": format!("{bin}/dwmapi.dll") }),
        ]);
    }
    let audit = audit_entry(&domain, &name, evidence, cases,
        "Identificação (executáveis, Steam ID, pasta do projeto) extraída da extensão; rotas do perfil Unreal compartilhado. Configurações em AppData, saves, filmes e substituição de paks originais da extensão não são aplicados.",
        nexus_game_id);
    Ok((module, audit))
}

fn identity(module: &Value) -> BTreeSet<String> {
    let mut key: BTreeSet<String> = module["required"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_ascii_lowercase).collect();
    key.extend(module["executable"].as_str().map(str::to_ascii_lowercase));
    key
}

/// Módulos gerados de uma família.
pub struct Generated {
    pub family: EmitFamily,
    pub modules: Vec<Value>,
    pub audits: Vec<Value>,
    pub skipped: Vec<Skipped>,
}

/// Gera os módulos de todas as famílias para jogos sem módulo próprio.
///
/// `existing_identities` são os arquivos exigidos pelos módulos atuais: uma
/// identidade contida em outra impediria a detecção dos dois jogos, então os
/// dois lados gerados são recusados.
pub fn generate_all(
    facts: &[GameFacts],
    existing: &BTreeSet<String>,
    existing_identities: &[(String, BTreeSet<String>)],
    evidence: &BTreeMap<String, String>,
    nexus: Option<&NexusIndex>,
) -> Vec<Generated> {
    let mut seen = existing.clone();
    let mut out: Vec<Generated> = EmitFamily::ALL.iter().map(|&family| generate(family, facts, &mut seen, evidence, nexus)).collect();
    let mut identities: Vec<(String, BTreeSet<String>)> = existing_identities.to_vec();
    for g in &out {
        identities.extend(g.modules.iter().map(|m| (m["domain"].as_str().unwrap_or_default().to_string(), identity(m))));
    }
    let mut ambiguous = BTreeMap::new();
    for (i, (a, ka)) in identities.iter().enumerate() {
        for (b, kb) in identities.iter().skip(i + 1) {
            if ka.is_subset(kb) || kb.is_subset(ka) {
                ambiguous.insert(a.clone(), b.clone());
                ambiguous.insert(b.clone(), a.clone());
            }
        }
    }
    for g in &mut out {
        let keep = |m: &Value| !ambiguous.contains_key(m["domain"].as_str().unwrap_or_default());
        for m in g.modules.iter().filter(|m| !keep(m)) {
            let domain = m["domain"].as_str().unwrap_or_default().to_string();
            let other = ambiguous[&domain].clone();
            g.skipped.push(Skipped { game: domain, reason: format!("identidade ambígua com {other}: mesmos executáveis exigidos") });
        }
        let modules = std::mem::take(&mut g.modules);
        let audits = std::mem::take(&mut g.audits);
        for (m, a) in modules.into_iter().zip(audits) {
            if keep(&m) {
                g.modules.push(m);
                g.audits.push(a);
            }
        }
    }
    out
}

fn generate(
    family: EmitFamily,
    facts: &[GameFacts],
    seen: &mut BTreeSet<String>,
    evidence: &BTreeMap<String, String>,
    nexus: Option<&NexusIndex>,
) -> Generated {
    let mut generated = Generated { family, modules: Vec::new(), audits: Vec::new(), skipped: Vec::new() };
    let mut candidates: Vec<&GameFacts> = facts.iter().filter(|f| family.matches(f)).collect();
    candidates.sort_by_key(|f| f.game_id.clone());
    for f in candidates {
        let id = f.nexus_domain.clone().or_else(|| f.game_id.clone()).unwrap_or_default();
        if !seen.insert(id.clone()) {
            continue;
        }
        let Some(base) = evidence.get(&f.source) else {
            generated.skipped.push(Skipped { game: id, reason: "fonte sem URL fixada".into() });
            continue;
        };
        let url = format!("{base}/{}/index.js", f.extension_dir);
        match family.module(f, &url, nexus) {
            Ok((m, a)) => {
                generated.modules.push(m);
                generated.audits.push(a);
            }
            Err(reason) => generated.skipped.push(Skipped { game: id, reason }),
        }
    }
    generated
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::extract::{Classification, Features, Level, ModType, StoreIds};

    fn facts() -> GameFacts {
        let mut stores = StoreIds::default();
        stores.steam.insert("3489700".into());
        GameFacts {
            source: "chemboy1".into(),
            extension_dir: "game-stellarblade".into(),
            game_id: Some("stellarblade".into()),
            name: Some("Stellar Blade".into()),
            nexus_domain: None,
            family: Some("Unreal Engine 4-5 Game".into()),
            stub_repository: None,
            executable: Some("SB.exe".into()),
            required_files: vec!["SB.exe".into()],
            mod_path: Some("SB/Content/Paks/~mods".into()),
            merge_mods: None,
            stores,
            stop_patterns: vec![],
            environment: vec![],
            mod_types: vec![ModType { id: "stellarblade-logicmods".into(), name: None, priority: None, target: Some("{gamePath}/SB/Content/Paks/LogicMods".into()) }],
            installers: vec![],
            tools: vec![],
            dependency_links: Default::default(),
            constants: [("SHIPPING_EXE", "SB/Binaries/Win64/SB-Win64-Shipping.exe"), ("EPIC_CODE_NAME", "SB")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            features: Features::default(),
            downloads: Vec::new(),
            unresolved: Default::default(),
            parse_errors: vec![],
            classification: Classification { level: Level::Yellow, reasons: vec![], game_data_complete: true },
        }
    }

    #[test]
    fn emits_family_module_with_strong_identity() {
        let (module, audit) = unreal_module(&facts(), "https://example/index.js", None).unwrap();
        assert_eq!(module["family"], json!({ "kind": "unreal", "project": "SB" }));
        assert_eq!(module["executable"], "SB/Binaries/Win64/SB-Win64-Shipping.exe");
        assert_eq!(module["launch_executable"], "SB.exe");
        assert_eq!(module["required"], json!(["SB.exe", "SB/Binaries/Win64/SB-Win64-Shipping.exe"]));
        assert_eq!(module["steam_id"], "3489700");
        assert!(audit["route_cases"].as_array().unwrap().iter().any(|c| c["target"] == "SB/Content/Paks/LogicMods/ExampleLogic.pak"));
    }

    #[test]
    fn refuses_unverifiable_identity() {
        let mut f = facts();
        f.constants.insert("SHIPPING_EXE".into(), "../SB-Win64-Shipping.exe".into());
        assert!(unreal_module(&f, "u", None).is_err());
        let mut f = facts();
        f.constants.insert("EPIC_CODE_NAME".into(), "Other".into());
        assert!(unreal_module(&f, "u", None).is_err());
        let mut f = facts();
        f.game_id = Some("stellar-blade".into());
        assert!(unreal_module(&f, "u", None).is_err());
        let index = NexusIndex { games: [("stellarblade".to_string(), 7804)].into(), with_extension: Default::default() };
        assert!(unreal_module(&facts(), "u", Some(&index)).unwrap_err().contains("extensão"));
        let index = NexusIndex { games: [("stellarblade".to_string(), 7804)].into(), with_extension: [7804].into() };
        assert_eq!(unreal_module(&facts(), "u", Some(&index)).unwrap().1["nexus_game_id"], 7804);
    }

    fn unity_facts(choice: &str, recommended: &str) -> GameFacts {
        let mut f = facts();
        f.game_id = Some("peak".into());
        f.name = Some("PEAK".into());
        f.family = Some("Unity BepinEx/MelonLoader/Custom Loader Hybrid".into());
        f.executable = Some("PEAK.exe".into());
        f.constants = [("DATA_FOLDER_DEFAULT", "PEAK_Data"), ("EXEC", "PEAK.exe"), ("loaderChoice", choice), ("recommendedLoader", recommended), ("isXna", "false"), ("ARCH", "x64")]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        f
    }

    #[test]
    fn emits_unity_module_with_extension_loaders() {
        let (module, audit) = unity_module(&unity_facts("false", "bep"), "u", None).unwrap();
        assert_eq!(module["family"], json!({ "kind": "unity", "data": "PEAK_Data", "loaders": ["bepinex"] }));
        assert_eq!(module["required"], json!(["PEAK.exe", "UnityPlayer.dll"]));
        assert!(audit["route_cases"].as_array().unwrap().iter().all(|c| !c["target"].as_str().unwrap().starts_with("Mods/")));
        let (module, _) = unity_module(&unity_facts("true", ""), "u", None).unwrap();
        assert_eq!(module["family"]["loaders"], json!(["bepinex", "melonloader"]));
        assert!(unity_module(&unity_facts("false", ""), "u", None).is_err());
        let mut xna = unity_facts("true", "bep");
        xna.constants.insert("isXna".into(), "true".into());
        assert!(unity_module(&xna, "u", None).is_err());
        let mut xbox = unity_facts("true", "bep");
        xbox.constants.insert("EXEC".into(), "gamelaunchhelper.exe".into());
        assert!(unity_module(&xbox, "u", None).is_err());
    }

    #[test]
    fn re_engine_uses_full_game_executable_not_demo() {
        let mut f = facts();
        f.game_id = Some("residentevil7".into());
        f.name = Some("Resident Evil 7".into());
        f.family = Some("3rd Party Mod Manager (Fluffy)".into());
        f.executable = Some("re7trial.exe".into());
        f.mod_types = vec![ModType { id: "residentevil7-reframework".into(), name: None, priority: None, target: Some("{gamePath}".into()) }];
        assert!(is_re_engine(&f));
        f.constants.insert("EXEC".into(), "re7.exe".into());
        let (module, _) = re_engine_module(&f, "u", None).unwrap();
        assert_eq!(module["required"], json!(["re7.exe"]));
        assert_eq!(module["family"], json!({ "kind": "re_engine" }));
        f.constants.remove("EXEC");
        assert!(re_engine_module(&f, "u", None).is_err(), "executável de demo recusado");
        f.mod_types.clear();
        assert!(!is_re_engine(&f), "sem REFramework na extensão");
    }

    #[test]
    fn ambiguous_generated_identities_are_both_refused() {
        let mut a = facts();
        a.game_id = Some("gamea".into());
        let mut b = facts();
        b.game_id = Some("gameb".into());
        b.extension_dir = "game-b".into();
        let evidence = [("chemboy1".to_string(), "https://example".to_string())].into();
        let out = generate_all(&[a, b], &BTreeSet::new(), &[], &evidence, None);
        let unreal = out.iter().find(|g| g.family == EmitFamily::Unreal).unwrap();
        assert!(unreal.modules.is_empty());
        assert_eq!(unreal.skipped.len(), 2);
    }
}
