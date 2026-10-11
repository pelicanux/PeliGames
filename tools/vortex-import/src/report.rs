//! Relatório Markdown da extração.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::path::PathBuf;

use crate::compare::{Comparison, PeliModule};
use crate::extract::{GameFacts, Level};

fn level_icon(l: Level) -> &'static str {
    match l {
        Level::Green => "🟢",
        Level::Yellow => "🟡",
        Level::Red => "🔴",
    }
}

fn cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn list(items: &[String]) -> String {
    if items.is_empty() { "—".into() } else { items.iter().map(|i| format!("`{}`", cell(i))).collect::<Vec<_>>().join(", ") }
}

pub fn render(sources: &[(String, PathBuf)], facts: &[GameFacts], modules: &[PeliModule], cmp: &[Comparison]) -> String {
    let mut o = String::new();
    let _ = writeln!(o, "# Importação das extensões Vortex — relatório\n");
    let _ = writeln!(
        o,
        "Gerado por `tools/vortex-import` por análise estática: nenhum código das extensões foi executado. \
         Os fatos indicam o que a extensão declara (detecção, destinos e reconhecimento de pacotes); \
         não comprovam que o mod carrega no jogo via Proton.\n"
    );
    let _ = writeln!(o, "Fontes:\n");
    for (name, dir) in sources {
        let _ = writeln!(o, "- `{name}`: `{}`", dir.display());
    }

    // Resumo por fonte e nível.
    let _ = writeln!(o, "\n## Resumo\n");
    let _ = writeln!(o, "| Fonte | Jogos | 🟢 convertível | 🟡 parcial | 🔴 manual |\n|---|---:|---:|---:|---:|");
    let mut by_source: BTreeMap<&str, [usize; 3]> = BTreeMap::new();
    for f in facts {
        let e = by_source.entry(&f.source).or_default();
        e[f.classification.level as usize] += 1;
    }
    for (src, c) in &by_source {
        let _ = writeln!(o, "| `{src}` | {} | {} | {} | {} |", c.iter().sum::<usize>(), c[0], c[1], c[2]);
    }
    let _ = writeln!(
        o,
        "\n- 🟢 **convertível**: jogo, lojas, executável e destinos resolvidos; instaladores com sinais estáticos.\n\
         - 🟡 **parcial**: algum dado depende de execução (destino calculado, teste sem sinais, diálogos) ou a família usa ordem de carregamento.\n\
         - 🔴 **manual**: merge, ferramenta externa ou registro não resolvido.\n"
    );

    // Famílias.
    let _ = writeln!(o, "## Famílias (campo `Structure` das extensões)\n");
    let _ = writeln!(
        o,
        "A coluna *dados do jogo* conta extensões com ID, loja, executável e ao menos um destino resolvidos. \
         Instaladores, ordem de carregamento e hooks pertencem à família e serão implementados uma vez por família.\n"
    );
    let mut families: BTreeMap<String, (usize, [usize; 3], usize)> = BTreeMap::new();
    for f in facts {
        let key = f.family.clone().unwrap_or_else(|| "(sem rótulo)".into());
        let e = families.entry(key).or_default();
        e.0 += 1;
        e.1[f.classification.level as usize] += 1;
        e.2 += usize::from(f.classification.game_data_complete);
    }
    let mut fam: Vec<_> = families.into_iter().collect();
    fam.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    let _ = writeln!(o, "| Família | Jogos | Dados do jogo | 🟢 | 🟡 | 🔴 |\n|---|---:|---:|---:|---:|---:|");
    for (name, (n, c, complete)) in fam {
        let _ = writeln!(o, "| {} | {n} | {complete} | {} | {} | {} |", cell(&name), c[0], c[1], c[2]);
    }

    // Comparação com os módulos atuais.
    let _ = writeln!(o, "\n## Módulos do PeliGames comparados ao Vortex\n");
    let _ = writeln!(
        o,
        "{} módulos lidos. Destinos comparados como caminhos relativos à raiz do jogo; \
         um destino é considerado compartilhado quando um contém o outro.\n",
        modules.len()
    );
    let _ = writeln!(o, "| Módulo | Extensão Vortex | Steam | Executável | Destinos em comum | Só no PeliGames | Só no Vortex |\n|---|---|:-:|:-:|---|---|---|");
    for c in cmp {
        let vortex = match (&c.vortex, &c.vortex_level, &c.matched_by) {
            (Some(v), Some(l), Some(by)) => {
                let icon = match l.as_str() { "green" => "🟢", "yellow" => "🟡", _ => "🔴" };
                format!("{icon} `{v}` ({by})")
            }
            _ => "**não encontrada**".into(),
        };
        let _ = writeln!(
            o,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            c.domain,
            vortex,
            c.steam.icon(),
            c.executable.icon(),
            list(&c.shared_destinations),
            list(&c.peligames_only),
            list(&c.vortex_only)
        );
    }

    // Jogos do Vortex ainda sem módulo.
    let covered: BTreeSet<String> = cmp.iter().filter_map(|c| c.vortex.clone()).collect();
    let mut missing: Vec<&GameFacts> = facts
        .iter()
        .filter(|f| !covered.contains(&f.key()))
        .filter(|f| f.game_id.is_some())
        .collect();
    missing.sort_by(|a, b| a.classification.level.cmp(&b.classification.level).then(a.game_id.cmp(&b.game_id)));
    let _ = writeln!(o, "\n## Jogos do Vortex sem módulo no PeliGames ({})\n", missing.len());
    let _ = writeln!(o, "| | Jogo | ID | Família | Steam | Destinos | Motivos |\n|---|---|---|---|---|---|---|");
    for f in missing {
        let mut dests: Vec<String> = f.mod_types.iter().filter_map(|m| m.target.clone()).collect();
        if let Some(mp) = &f.mod_path {
            dests.insert(0, mp.clone());
        }
        dests.dedup();
        dests.truncate(4);
        let _ = writeln!(
            o,
            "| {} | {} | `{}` | {} | {} | {} | {} |",
            level_icon(f.classification.level),
            cell(f.name.as_deref().unwrap_or("?")),
            f.game_id.as_deref().unwrap_or("?"),
            cell(f.family.as_deref().unwrap_or("—")),
            f.stores.steam.iter().next().cloned().unwrap_or_else(|| "—".into()),
            list(&dests),
            cell(&f.classification.reasons.join("; "))
        );
    }

    // Problemas de leitura.
    let unresolved: Vec<&GameFacts> = facts.iter().filter(|f| f.game_id.is_none() || !f.parse_errors.is_empty()).collect();
    if !unresolved.is_empty() {
        let _ = writeln!(o, "\n## Extensões não lidas por completo ({})\n", unresolved.len());
        for f in unresolved {
            let mut why = f.classification.reasons.clone();
            why.extend(f.parse_errors.iter().cloned());
            if let Some(stub) = &f.stub_repository {
                why.push(format!("repositório: {stub}"));
            }
            let _ = writeln!(o, "- `{}`: {}", f.key(), why.join("; "));
        }
    }
    o
}
