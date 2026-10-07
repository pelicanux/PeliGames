//! Default library location and optional game folder preparation.
#[tauri::command]
pub fn prepare_default_installation_directory(folder: Option<String>) -> Result<String, String> {
    let home = dirs::home_dir().ok_or("Não foi possível localizar a pasta pessoal.")?;
    let mut directory = home.join("Games").join("PeliGames");
    if let Some(folder) = folder {
        if folder.is_empty()
            || folder == "."
            || folder == ".."
            || folder.contains('/')
            || folder.contains('\\')
        {
            return Err("Nome de pasta inválido.".into());
        }
        directory.push(folder);
    }
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Não foi possível criar a pasta padrão: {error}"))?;
    Ok(directory.to_string_lossy().into_owned())
}
