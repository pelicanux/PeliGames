//! Contextual support snapshots. No portal prompts, installs, media or network I/O.
mod collectors;
mod files;
mod projection;
use crate::core::{paths, report_privacy::sanitize};
use collectors::{captures, frame_profiles, neural, system};
use files::{read_at, section, technical};
use projection::advanced;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, io::Write, path::Path};
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Dlssnr,
    FrameGeneration,
    Performance,
    Captures,
    Options,
    Advanced,
}
impl Category {
    fn label(self) -> &'static str {
        match self {
            Self::Dlssnr => "DLSSNR-AMD",
            Self::FrameGeneration => "Geração de quadros",
            Self::Performance => "Desempenho e overlay",
            Self::Captures => "Capturas",
            Self::Options => "Opções de execução",
            Self::Advanced => "Avançado",
        }
    }
    fn slug(self) -> &'static str {
        match self {
            Self::Dlssnr => "dlssnr",
            Self::FrameGeneration => "geracao-de-quadros",
            Self::Performance => "desempenho",
            Self::Captures => "capturas",
            Self::Options => "opcoes",
            Self::Advanced => "avancado",
        }
    }
}
#[derive(Deserialize)]
pub struct Context {
    category: Category,
    name: String,
    path: String,
    directory: String,
    #[serde(default)]
    mod_directory: String,
    capture_key: String,
    launcher: String,
    #[serde(default)]
    proton: String,
    #[serde(default)]
    nexus_id: Option<String>,
    #[serde(default)]
    advanced: Value,
    #[serde(default)]
    native: bool,
    #[serde(default)]
    flatpak: bool,
}
const REPORT_LIMIT: usize = 512 * 1024;
fn build(context: &Context, label: &str) -> String {
    let mut out=format!("PELIGAMES — RELATÓRIO\nJogo: {}\nÁrea: {}\nLauncher: {} (backend {})\nVersão: {}\nOrigem: {}\nNativo Linux: {}\nFlatpak: {}\nData Unix: {}\n\nAjustes configurados; não comprovam ativação no jogo. Logs podem pertencer à última execução.\n",
        context.name.replace(['\n','\r']," "),context.category.label(),label,env!("CARGO_PKG_VERSION"),super::official_build::report_label(),context.launcher,context.native,context.flatpak,std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs());
    system(&mut out);
    if !context.proton.is_empty() {
        let proton = Path::new(&context.proton);
        section(
            &mut out,
            "PROTON",
            &format!(
                "{}\n{}",
                proton.file_name().unwrap_or_default().to_string_lossy(),
                read_at(proton, "version").unwrap_or_else(|| "Versão não disponível.".into())
            ),
        );
    }
    technical(
        &mut out,
        "CONFIGURAÇÃO DE EXECUÇÃO",
        &advanced(&context.advanced),
    );
    match context.category {
        Category::Dlssnr => neural(
            &mut out,
            Path::new(if context.mod_directory.is_empty() {
                &context.directory
            } else {
                &context.mod_directory
            }),
        ),
        Category::Captures => captures(&mut out, &context.capture_key),
        Category::FrameGeneration => {
            frame_profiles(&mut out, &context.advanced);
            neural(&mut out, Path::new(&context.directory));
        }
        Category::Performance => {
            let settings = context
                .advanced
                .get("mangohud_preset")
                .and_then(|p| p.get("settings"));
            if let Some(settings) = settings {
                let safe = settings
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter(|(_, v)| v.is_boolean() || v.is_number())
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<serde_json::Map<_, _>>();
                technical(
                    &mut out,
                    "MANGOHUD (preset deste jogo, sem nome pessoal)",
                    &safe.into(),
                );
            } else if let Ok(profile) = crate::core::mangohud_config::load() {
                technical(
                    &mut out,
                    "MANGOHUD (valores numéricos e indicadores)",
                    &serde_json::to_value(
                        profile
                            .settings
                            .into_iter()
                            .filter(|(_, v)| v.is_boolean() || v.is_number())
                            .collect::<std::collections::BTreeMap<_, _>>(),
                    )
                    .unwrap_or_default(),
                );
            }
        }
        Category::Options | Category::Advanced => {}
    }
    if let Some(session) =
        super::frame_overlay_runtime::report_session(Path::new(&context.directory))
    {
        technical(
            &mut out,
            "ÚLTIMA SESSÃO DO OVERLAY",
            &serde_json::to_value(session).unwrap_or_default(),
        );
    }
    // Resolve by installation identity; never take the newest unrelated game's log.
    if let Some(game) = context
        .nexus_id
        .as_ref()
        .and_then(|id| super::nexus_local::diagnostic_game(id).ok())
        .filter(|game| projection::same_installation(&game.game, &context.path, &context.directory))
    {
        for previous in [false, true] {
            if let Ok(text) = super::nexus_profile::logs(&game, previous) {
                section(
                    &mut out,
                    if previous {
                        "EXECUÇÃO ANTERIOR"
                    } else {
                        "EXECUÇÃO ATUAL"
                    },
                    &text,
                );
            }
        }
    } else if let Ok(logs) = crate::core::game_logs::read(&context.path) {
        section(&mut out, "EXECUÇÃO ATUAL", &logs.current);
        section(&mut out, "EXECUÇÃO ANTERIOR", &logs.previous);
    }
    sanitize(&out)
}
fn save(context: &Context, label: &str) -> Result<super::nexus_reports::Report, String> {
    let mut content = build(context, label);
    let truncated = content.len() > REPORT_LIMIT;
    if truncated {
        let mut end = REPORT_LIMIT;
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        content.truncate(end);
        content.push_str("\n[Relatório reduzido.]\n");
    }
    let root = paths::app_root()?;
    // Validate each directory before writing, including existing components.
    let mut dir = root.clone();
    for part in ["logs", "reports"] {
        dir.push(part);
        match fs::symlink_metadata(&dir) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {}
            Ok(_) => return Err("Pasta de relatórios inválida.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                use std::os::unix::fs::DirBuilderExt;
                fs::DirBuilder::new()
                    .mode(0o700)
                    .create(&dir)
                    .map_err(|e| e.to_string())?;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let label = format!("{} {}", context.name, context.category.slug());
    let identity = format!("{}:{}", context.path, context.category.slug());
    let path = paths::names::allocate(&dir, &label, &identity)?.with_extension("txt");
    let temp = dir.join(format!(".relatorio-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        file.write_all(content.as_bytes())
            .and_then(|_| file.sync_all())
            .map_err(|e| e.to_string())?;
        fs::rename(&temp, &path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result?;
    Ok(super::nexus_reports::Report {
        path: path.to_string_lossy().into_owned(),
        bytes: content.len(),
        content,
        truncated,
    })
}
#[tauri::command]
pub async fn export_feature_report(
    context: Context,
    build_label: String,
) -> Result<super::nexus_reports::Report, String> {
    if context.name.len() > 256
        || context.path.len() > 4096
        || context.directory.len() > 4096
        || context.mod_directory.len() > 4096
        || serde_json::to_vec(&context.advanced)
            .map_err(|e| e.to_string())?
            .len()
            > 64 * 1024
    {
        return Err("Contexto de relatório inválido.".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        save(&context, &build_label.chars().take(64).collect::<String>())
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_categories_are_rejected_and_utf8_tail_is_safe() {
        assert!(serde_json::from_str::<Category>("\"everything\"").is_err());
        assert_eq!(files::tail("áéabc", 4), "abc");
    }
}
