#[tauri::command]
pub fn system_language_preferences() -> Vec<String> {
    crate::core::system_locale::preferences(|name| std::env::var(name).ok())
}
