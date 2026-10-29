//! Shared Discord bridge. Preparing a submission never makes a network request.
use crate::core::report_privacy::sanitize;
use serde::{Deserialize, Serialize};
const DEFAULT_RELAY: &str = "https://peligames-report.nairomarttins97.workers.dev";
pub(super) const MAX_REPORT_BYTES: usize = 9_000_000;
fn relay() -> Option<&'static str> {
    option_env!("PELIGAMES_REPORT_RELAY")
        .or(Some(DEFAULT_RELAY))
        .filter(|s| s.starts_with("https://"))
}
#[tauri::command]
pub fn report_relay_configured() -> bool {
    relay().is_some()
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Submission {
    title: String,
    description: String,
    discord: String,
    version: String,
    report: String,
}
#[tauri::command]
pub fn prepare_report_submission(
    title: String,
    description: String,
    discord: String,
    version: String,
    report: String,
) -> Result<Submission, String> {
    if report.len() > MAX_REPORT_BYTES || !report.starts_with("PELIGAMES — RELATÓRIO") {
        return Err("Relatório inválido ou grande demais.".into());
    }
    let title = sanitize(&title.chars().take(100).collect::<String>()).replace('\n', " ");
    if title.trim().is_empty() {
        return Err("Informe o título do relatório.".into());
    }
    Ok(Submission {
        title,
        description: sanitize(&description.chars().take(1500).collect::<String>()),
        discord: sanitize(&discord.chars().take(40).collect::<String>()).replace('\n', " "),
        version: sanitize(&version.chars().take(64).collect::<String>()).replace('\n', " "),
        report: sanitize(&report),
    })
}
#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    error: Option<String>,
}
#[tauri::command]
pub async fn send_reviewed_report(submission: Submission) -> Result<Option<String>, String> {
    // Revalidate at the boundary; never recollect logs after review.
    let submission = prepare_report_submission(
        submission.title,
        submission.description,
        submission.discord,
        submission.version,
        submission.report,
    )?;
    let relay = relay().ok_or("O envio direto não está configurado. Use o canal do Discord.")?;
    let client = reqwest::Client::builder()
        .user_agent("PeliGames")
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| e.to_string())?;
    let response = client
        .post(format!("{}/report", relay.trim_end_matches('/')))
        .json(&submission)
        .send()
        .await
        .map_err(|_| {
            "Não foi possível falar com o servidor de relatórios. Verifique a conexão.".to_string()
        })?;
    let status = response.status();
    let reply = response.json::<Reply>().await.unwrap_or(Reply {
        url: None,
        error: None,
    });
    if !status.is_success() {
        return Err(reply
            .error
            .map(|e| sanitize(&e))
            .unwrap_or_else(|| format!("O envio falhou ({status}).")));
    }
    Ok(reply.url.filter(|s| s.starts_with("https://discord.com/")))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reviews_and_sends_same_sanitized_snapshot_without_recollection() {
        let first = prepare_report_submission(
            "Erro token=hidden".into(),
            "Senha: hidden".into(),
            "Pelicano".into(),
            "0.5.1".into(),
            "PELIGAMES — RELATÓRIO\nJogo: Teste\n/opt/private/name\nFPS: 60".into(),
        )
        .unwrap();
        let second = prepare_report_submission(
            first.title.clone(),
            first.description.clone(),
            first.discord.clone(),
            first.version.clone(),
            first.report.clone(),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(second).unwrap()
        );
        assert!(!first.report.contains("private"));
        assert!(!first.description.contains("hidden"));
    }
    #[test]
    fn rejects_foreign_payloads_before_any_network_operation() {
        assert!(prepare_report_submission(
            "test".into(),
            "".into(),
            "".into(),
            "".into(),
            "foreign".into()
        )
        .is_err());
        assert!(prepare_report_submission(
            "test".into(),
            "".into(),
            "".into(),
            "".into(),
            "x".repeat(MAX_REPORT_BYTES + 1)
        )
        .is_err());
    }
}
