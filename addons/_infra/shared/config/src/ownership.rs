use std::path::Path;

/// Shared naming contract for integration-owned installation receipts in games.
pub fn receipt_name(integration: &str) -> Option<String> {
    if integration.is_empty()
        || !integration
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return None;
    }
    Some(format!("peligames-{integration}-install.json"))
}
pub fn installed(directory: &Path, integration: &str) -> bool {
    receipt_name(integration).is_some_and(|name| directory.join(name).is_file())
}
