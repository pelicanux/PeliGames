//! peligames-vortex-import
//!
//! Lê extensões de jogos do Vortex por análise estática (sem executá-las),
//! extrai os fatos declarativos de cada jogo e compara com os módulos Nexus
//! do PeliGames. O resultado é um conjunto de JSONs e um relatório Markdown;
//! nenhum módulo do launcher é alterado.

mod compare;
mod emit;
mod extract;
mod js;
mod report;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use extract::{extract_extension, GameFacts};

const USAGE: &str = "Uso:
  peligames-vortex-import --source NOME=DIR [--source NOME=DIR ...] \\
      [--modules src-tauri/resources/nexus-games] [--out tools/vortex-import/generated] \\
      [--evidence NOME=URL_BASE ...] [--emit DIR] [--nexus-index GAMES.json MANIFESTO.json]

--emit grava, por família (unreal, unity), DIR/<família>-vortex.json (módulos) e
DIR/<família>-vortex-audit.json para os jogos sem módulo próprio. --evidence informa a URL fixada de cada fonte.
--nexus-index valida cada domínio na lista pública de jogos do Nexus e exige uma
extensão Vortex publicada no manifesto público (api.nexusmods.com/v3/vortex/extensions).

Cada DIR pode ser uma pasta com várias extensões (subpastas `game-*`) ou uma
única extensão (repositório próprio de um jogo).";

struct Args {
    sources: Vec<(String, PathBuf)>,
    modules: PathBuf,
    out: PathBuf,
    evidence: std::collections::BTreeMap<String, String>,
    emit: Option<PathBuf>,
    nexus_index: Option<(PathBuf, PathBuf)>,
}

/// Arquivos regravados por `--emit` (`<família>-vortex.json`).
fn is_generated(file: &str) -> bool {
    emit::EmitFamily::ALL.iter().any(|f| file == format!("{}-vortex.json", f.name()))
}

fn parse_args() -> Result<Args, String> {
    let mut sources = Vec::new();
    let mut modules = PathBuf::from("src-tauri/resources/nexus-games");
    let mut out = PathBuf::from("tools/vortex-import/generated");
    let mut evidence = std::collections::BTreeMap::new();
    let mut emit = None;
    let mut nexus_index = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--source" => {
                let v = it.next().ok_or("--source exige NOME=DIR")?;
                let (name, dir) = v.split_once('=').ok_or("--source exige NOME=DIR")?;
                sources.push((name.to_string(), PathBuf::from(dir)));
            }
            "--modules" => modules = PathBuf::from(it.next().ok_or("--modules exige um diretório")?),
            "--out" => out = PathBuf::from(it.next().ok_or("--out exige um diretório")?),
            "--evidence" => {
                let v = it.next().ok_or("--evidence exige NOME=URL")?;
                let (name, url) = v.split_once('=').ok_or("--evidence exige NOME=URL")?;
                evidence.insert(name.to_string(), url.trim_end_matches('/').to_string());
            }
            "--nexus-index" => {
                let games = PathBuf::from(it.next().ok_or("--nexus-index exige GAMES.json MANIFESTO.json")?);
                let manifest = PathBuf::from(it.next().ok_or("--nexus-index exige GAMES.json MANIFESTO.json")?);
                nexus_index = Some((games, manifest));
            }
            "--emit" => emit = Some(PathBuf::from(it.next().ok_or("--emit exige um diretório")?)),
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("argumento desconhecido: {other}\n\n{USAGE}")),
        }
    }
    if sources.is_empty() {
        return Err(USAGE.to_string());
    }
    Ok(Args { sources, modules, out, evidence, emit, nexus_index })
}

fn extension_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut subdirs: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir() && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("game-")))
                .collect()
        })
        .unwrap_or_default();
    subdirs.sort();
    if subdirs.is_empty() {
        vec![dir.to_path_buf()]
    } else {
        subdirs
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let mut all: Vec<GameFacts> = Vec::new();
    for (name, dir) in &args.sources {
        if !dir.is_dir() {
            return Err(format!("fonte {name}: diretório inexistente: {}", dir.display()));
        }
        let facts_dir = args.out.join("facts").join(name);
        fs::create_dir_all(&facts_dir).map_err(|e| format!("{}: {e}", facts_dir.display()))?;
        let dirs = extension_dirs(dir);
        eprintln!("{name}: {} extensão(ões)", dirs.len());
        for ext_dir in dirs {
            let games = extract_extension(name, &ext_dir);
            let multiple = games.len() > 1;
            for facts in games {
                let file_name = match (&facts.game_id, multiple) {
                    (Some(id), true) => format!("{}--{id}.json", facts.extension_dir),
                    _ => format!("{}.json", facts.extension_dir),
                };
                let file = facts_dir.join(file_name);
                let json = serde_json::to_string_pretty(&facts).map_err(|e| e.to_string())?;
                fs::write(&file, json + "\n").map_err(|e| format!("{}: {e}", file.display()))?;
                all.push(facts);
            }
        }
    }

    if let Some(dir) = &args.emit {
        // Módulos próprios (não gerados) têm prioridade sobre a geração.
        let own: Vec<compare::PeliModule> = compare::load_modules(&args.modules)?.into_iter().filter(|m| !is_generated(&m.file)).collect();
        let existing: std::collections::BTreeSet<String> = own.iter().map(|m| m.domain.clone()).collect();
        let identities: Vec<_> = own
            .iter()
            .map(|m| (m.domain.clone(), m.required.iter().chain(m.executable.iter()).map(|p| p.to_ascii_lowercase()).collect()))
            .collect();
        let index = match &args.nexus_index {
            Some((games, manifest)) => Some(emit::NexusIndex::load(games, manifest)?),
            None => None,
        };
        let write = |name: &str, value: &serde_json::Value| -> Result<(), String> {
            let path = dir.join(name);
            fs::write(&path, serde_json::to_string_pretty(value).map_err(|e| e.to_string())? + "\n").map_err(|e| format!("{}: {e}", path.display()))
        };
        for generated in emit::generate_all(&all, &existing, &identities, &args.evidence, index.as_ref()) {
            let family = generated.family.name();
            write(&format!("{family}-vortex.json"), &serde_json::Value::Array(generated.modules.clone()))?;
            write(&format!("{family}-vortex-audit.json"), &serde_json::json!({
                "schema": 1,
                "generated_by": "tools/vortex-import --emit",
                "family": family,
                "scope": "Módulos gerados por análise estática das extensões Vortex. Rotas verificadas por casos sintéticos; nenhum jogo foi executado.",
                "sources": args.evidence,
                "modules": generated.audits,
                "skipped": generated.skipped.iter().map(|s| serde_json::json!({ "domain": s.game, "reason": s.reason })).collect::<Vec<_>>(),
            }))?;
            eprintln!("{family}: {} módulo(s) gerado(s), {} recusado(s)", generated.modules.len(), generated.skipped.len());
        }
    }

    let modules = compare::load_modules(&args.modules)?;
    let comparisons = compare::compare(&modules, &all);

    let summary = serde_json::json!({
        "sources": args.sources.iter().map(|(n, d)| serde_json::json!({ "name": n, "dir": d })).collect::<Vec<_>>(),
        "extensions": all.len(),
        "comparisons": comparisons,
    });
    let summary_path = args.out.join("summary.json");
    fs::write(&summary_path, serde_json::to_string_pretty(&summary).map_err(|e| e.to_string())? + "\n")
        .map_err(|e| format!("{}: {e}", summary_path.display()))?;

    let report_path = args.out.join("report.md");
    fs::write(&report_path, report::render(&args.sources, &all, &modules, &comparisons))
        .map_err(|e| format!("{}: {e}", report_path.display()))?;
    eprintln!("relatório: {}", report_path.display());
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
