//! Save games and per-profile game settings of the Bethesda games, as the
//! Vortex gamebryo-savegame-management and local-gamesettings extensions:
//! save headers (character, location, plugins, screenshot), saves per profile
//! through `SLocalSavePath`, and INIs swapped with the active profile.
use super::nexus_local::NexusGame;
use super::nexus_modules::{Definition, SaveRules};
use std::{fs, io::Read, path::{Component, Path, PathBuf}};

/// What the save page shows about one save.
#[derive(serde::Serialize, Clone, Debug, PartialEq)]
pub struct SaveInfo {
    pub file: String,
    pub character: String,
    pub level: u32,
    pub location: String,
    pub playtime: String,
    pub number: u32,
    /// Unix seconds (save header, else file date).
    pub created: u64,
    pub plugins: Vec<String>,
}
pub(super) struct Screenshot { pub width: u32, pub height: u32, pub rgba: Vec<u8> }

struct Reader<'a> { buf: std::borrow::Cow<'a, [u8]>, pos: usize, markers: bool, bz: bool }
impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self { Reader { buf: std::borrow::Cow::Borrowed(buf), pos: 0, markers: false, bz: false } }
    fn bytes(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos.checked_add(n).filter(|end| *end <= self.buf.len()).ok_or("Save truncado.")?;
        let start = self.pos;
        self.pos = end;
        Ok(&self.buf[start..end])
    }
    fn skip(&mut self, n: usize) -> Result<(), String> { self.bytes(n).map(|_| ()) }
    /// Fallout 3/New Vegas headers put a `|` after each field.
    fn marker(&mut self) -> Result<(), String> {
        if self.markers && self.pos < self.buf.len() {
            if self.buf[self.pos] != b'|' { return Err("Separador de campo ausente no save.".into()); }
            self.pos += 1;
        }
        Ok(())
    }
    fn u8(&mut self) -> Result<u8, String> { let v = self.bytes(1)?[0]; self.marker()?; Ok(v) }
    fn u16(&mut self) -> Result<u16, String> { let v = u16::from_le_bytes(self.bytes(2)?.try_into().unwrap()); self.marker()?; Ok(v) }
    fn u32(&mut self) -> Result<u32, String> { let v = u32::from_le_bytes(self.bytes(4)?.try_into().unwrap()); self.marker()?; Ok(v) }
    fn f32(&mut self) -> Result<f32, String> { let v = f32::from_le_bytes(self.bytes(4)?.try_into().unwrap()); self.marker()?; Ok(v) }
    fn i64(&mut self) -> Result<i64, String> { let v = i64::from_le_bytes(self.bytes(8)?.try_into().unwrap()); self.marker()?; Ok(v) }
    fn text(raw: &[u8]) -> String {
        match std::str::from_utf8(raw) { Ok(text) => text.to_string(), Err(_) => raw.iter().map(|b| *b as char).collect() }
    }
    /// `u16` length (or `u8` with a trailing NUL in Oblivion), then the text.
    fn string(&mut self) -> Result<String, String> {
        let length = if self.bz { self.u8()? as usize } else { self.u16()? as usize };
        if length == 0 { return Ok(String::new()); }
        let raw = self.bytes(length)?.to_vec();
        let text = Self::text(if self.bz { &raw[..raw.len() - 1] } else { &raw });
        self.marker()?;
        Ok(text)
    }
    fn bstring(&mut self) -> Result<String, String> {
        let length = self.bytes(1)?[0] as usize;
        Ok(Self::text(&self.bytes(length)?.to_vec()))
    }
    fn image(&mut self, size: Option<(u32, u32)>, alpha: bool, keep: bool) -> Result<Option<Screenshot>, String> {
        let (width, height) = match size { Some(size) => size, None => (self.u32()?, self.u32()?) };
        if width >= 2000 || height >= 2000 { return Err("Miniatura do save inválida.".into()); }
        let raw = self.bytes(width as usize * height as usize * if alpha { 4 } else { 3 })?;
        if !keep { return Ok(None); }
        let rgba = if alpha { raw.to_vec() } else { raw.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect() };
        Ok(Some(Screenshot { width, height, rgba }))
    }
    fn plugins(&mut self, bstrings: bool) -> Result<Vec<String>, String> {
        let count = self.u8()?;
        (0..count).map(|_| if bstrings { self.bstring() } else { self.string() }).collect()
    }
    fn light_plugins(&mut self) -> Result<Vec<String>, String> {
        let count = self.u16()?;
        (0..count).map(|_| self.string()).collect()
    }
}
fn filetime(ticks: i64) -> u64 { (ticks / 10_000_000).saturating_sub(11_644_473_600).max(0) as u64 }
/// Reads a save header (Vortex GamebryoSaveGame): Oblivion, Skyrim LE/SE/VR,
/// Fallout 3/New Vegas and Fallout 4. Only the beginning of the file is read.
pub(super) fn parse(path: &Path, screenshot: bool) -> Result<(SaveInfo, Option<Screenshot>), String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    file.by_ref().take(12 * 1024 * 1024).read_to_end(&mut buf).map_err(|e| e.to_string())?;
    let mut r = Reader::new(&buf);
    let header = |name: &str| buf.starts_with(name.as_bytes());
    let (number, character, level, location, playtime, mut created, image, plugins);
    if header("TES4SAVEGAME") {
        r.skip(12)?; r.bz = true;
        r.skip(1 + 1 + 16 + 4 + 4)?;
        number = r.u32()?; character = r.string()?; level = r.u16()? as u32; location = r.string()?;
        let days = r.f32()?;
        playtime = format!("{} days, {} hours", days.floor() as i64, (days * 24.0).floor() as i64 % 24);
        r.skip(4)?;
        let time: Vec<u16> = (0..8).map(|_| r.u16()).collect::<Result<_, _>>()?;
        created = civil(time[0] as i64, time[1] as i64, time[3] as i64, time[4] as i64, time[5] as i64, time[6] as i64);
        r.skip(4)?;
        image = r.image(None, false, screenshot)?;
        plugins = r.plugins(true)?;
    } else if header("TESV_SAVEGAME") || header("FO4_SAVEGAME") {
        let fo4 = header("FO4_SAVEGAME");
        r.skip(if fo4 { 12 } else { 13 })?;
        r.skip(4)?;
        let version = r.u32()?;
        number = r.u32()?; character = r.string()?; level = r.u32()?; location = r.string()?; playtime = r.string()?;
        r.string()?; r.skip(2 + 4 + 4)?;
        created = Some(filetime(r.i64()?));
        let mut light = Vec::new();
        let list;
        if fo4 {
            image = r.image(None, true, screenshot)?;
            let form = r.u8()?; r.string()?; r.skip(4)?;
            list = r.plugins(false)?;
            if form >= 0x44 { light = r.light_plugins()?; }
        } else if version < 0x0c {
            image = r.image(None, false, screenshot)?;
            r.u8()?; r.skip(4)?;
            list = r.plugins(false)?;
        } else {
            let (width, height) = (r.u32()?, r.u32()?);
            let compression = r.u16()?;
            image = r.image(Some((width, height)), true, screenshot)?;
            let (uncompressed, compressed) = (r.u32()? as usize, r.u32()? as usize);
            let rest = &buf[r.pos..];
            let input = &rest[..compressed.min(rest.len())];
            // The plugin list sits at the start of the compressed body.
            let want = uncompressed.min(256 * 1024);
            let body = match compression {
                0 => input[..want.min(input.len())].to_vec(),
                1 => { let mut out = Vec::new(); flate2::read::ZlibDecoder::new(input).take(want as u64).read_to_end(&mut out).map_err(|e| e.to_string())?; out }
                2 => super::nexus_bg3::lz4_prefix(input, want)?,
                _ => return Err("Compressão do save desconhecida.".into()),
            };
            let mut inner = Reader { buf: std::borrow::Cow::Owned(body), pos: 0, markers: false, bz: false };
            let form = inner.u8()?; inner.skip(4)?;
            list = inner.plugins(false)?;
            if form >= 0x4e { light = inner.light_plugins()?; }
        }
        plugins = list.into_iter().chain(light).collect();
    } else if header("FO3SAVEGAME") {
        r.skip(11)?;
        r.skip(4 + 4 + 1)?;
        // New Vegas has an extra field before the screenshot size.
        let start = r.pos;
        let mut field = 0;
        while r.u8()? != b'|' { field += 1; }
        if field == 4 { r.pos = start; }
        r.markers = true;
        let (width, height) = (r.u32()?, r.u32()?);
        number = r.u32()?; character = r.string()?; r.string()?;
        level = r.u32()?; location = r.string()?; playtime = r.string()?;
        created = None;
        image = r.image(Some((width, height)), false, screenshot)?;
        r.skip(5)?;
        plugins = r.plugins(false)?;
    } else {
        return Err("Formato de save desconhecido.".into());
    }
    if created.is_none_or(|time| time == 0) {
        created = fs::metadata(path).and_then(|m| m.modified()).ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs());
    }
    let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok((SaveInfo { file, character, level, location, playtime, number, created: created.unwrap_or(0), plugins }, image))
}
fn civil(year: i64, month: i64, day: i64, hour: i64, minute: i64, second: i64) -> Option<u64> {
    let date = super::nexus_gamebryo::date(&format!("{year:04}-{month:02}-{day:02}"))?;
    Some(date.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs() + (hour * 3600 + minute * 60 + second).max(0) as u64)
}
/// PNG data URL of a screenshot.
pub(super) fn png_url(shot: &Screenshot) -> Result<String, String> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, shot.width, shot.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().and_then(|mut writer| writer.write_image_data(&shot.rgba)).map_err(|e| e.to_string())?;
    }
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::from("data:image/png;base64,");
    for chunk in bytes.chunks(3) {
        let value = (chunk[0] as u32) << 16 | (*chunk.get(1).unwrap_or(&0) as u32) << 8 | *chunk.get(2).unwrap_or(&0) as u32;
        for index in 0..4 {
            text.push(if index <= chunk.len() { TABLE[(value >> (18 - 6 * index) & 63) as usize] as char } else { '=' });
        }
    }
    Ok(text)
}
pub(super) fn rules(module: &Definition) -> Result<&SaveRules, String> {
    module.saves.as_ref().ok_or_else(|| "Este jogo não tem gerenciamento de saves.".into())
}
/// Folder `My Games/<game>` inside the prefix.
fn folder(game: &NexusGame, rules: &SaveRules) -> Result<PathBuf, String> {
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let (base, relative) = super::nexus_targets::root(root, &game.compat_data, &format!("{}probe", rules.folder))?;
    Ok(base.join(relative).parent().ok_or("Pasta de saves inválida.")?.to_path_buf())
}
/// `SLocalSavePath` value of a profile: the shared saves, or `<base>\<profile>\`.
pub(super) fn save_path_value(rules: &SaveRules, profile: Option<&str>) -> String {
    match profile { Some(id) => format!("{}\\{id}\\", rules.base), None => format!("{}\\", rules.base) }
}
/// Folder of the saves for a profile, resolved like the game does.
fn saves_dir(game: &NexusGame, rules: &SaveRules, profile: Option<&str>) -> Result<PathBuf, String> {
    let mut path = folder(game, rules)?;
    for part in save_path_value(rules, profile).split('\\').filter(|part| !part.is_empty()) {
        match part { ".." => { path.pop(); } "." => {} part => path.push(part) }
    }
    if path.components().any(|c| matches!(c, Component::ParentDir)) { return Err("Pasta de saves inválida.".into()); }
    Ok(path)
}
/// Writes `[General] SLocalSavePath` for the active profile. Only a value
/// written by PeliGames is put back to the shared folder.
pub(super) fn apply_save_path(game: &NexusGame, module: &Definition, own: Option<&str>) -> Result<(), String> {
    let Ok(rules) = rules(module) else { return Ok(()); };
    let ini = folder(game, rules)?.join(&rules.ini);
    let text = fs::read(&ini).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default();
    let current = text.lines().scan(false, |inside, line| {
        let trimmed = line.trim();
        if trimmed.starts_with('[') { *inside = trimmed.eq_ignore_ascii_case("[General]"); }
        Some((*inside).then(|| trimmed.split_once('=').filter(|(key, _)| key.trim().eq_ignore_ascii_case("SLocalSavePath")).map(|(_, value)| value.trim().to_string())).flatten())
    }).flatten().next();
    let ours = current.as_deref().is_some_and(|value| value.strip_prefix(&format!("{}\\", rules.base)).and_then(|rest| rest.strip_suffix('\\'))
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok() || id == super::nexus_user_profiles::DEFAULT_ID));
    if own.is_none() && !ours { return Ok(()); }
    if text.trim().is_empty() && !rules.optional.iter().any(|file| file.eq_ignore_ascii_case(&rules.ini)) {
        return Err(format!("Inicie {} uma vez no prefixo para criar {} antes de usar saves por perfil.", module.name, rules.ini));
    }
    let value = save_path_value(rules, own);
    let output = super::nexus_activation::ini(&text, "General", &[format!("SLocalSavePath={value}")], |line| line.split_once('=').is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("SLocalSavePath")));
    fs::create_dir_all(ini.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
    fs::write(&ini, output).map_err(|e| e.to_string())?;
    if let Some(id) = own { fs::create_dir_all(saves_dir(game, rules, Some(id))?).map_err(|e| e.to_string())?; }
    Ok(())
}
fn settings_files(rules: &SaveRules) -> impl Iterator<Item = (&String, bool)> {
    rules.settings.iter().map(|file| (file, false)).chain(rules.optional.iter().map(|file| (file, true)))
}
fn store(game: &NexusGame, name: &str) -> Result<PathBuf, String> {
    Ok(super::nexus_profile::directory(game)?.join("user-profiles").join(name).join("settings"))
}
/// Copies the game settings between the live folder and a stored copy.
fn copy_settings(rules: &SaveRules, from: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| e.to_string())?;
    for (file, optional) in settings_files(rules) {
        match super::nexus_discovery::locate(from, file) {
            Some(source) if source.is_file() => { fs::copy(&source, to.join(file)).map_err(|e| format!("Falha ao copiar {file}: {e}"))?; }
            _ if optional => { let _ = fs::remove_file(to.join(file)); }
            _ => {}
        }
    }
    Ok(())
}
/// Leaves a profile with its own settings (its INIs are kept, the global ones
/// come back) and enters the next one (the global INIs are kept aside, the
/// profile's are put in place; a new profile starts from the global ones).
pub(super) fn swap_settings(game: &NexusGame, module: &Definition, leaving: Option<&str>, entering: Option<&str>) -> Result<(), String> {
    let Ok(rules) = rules(module) else { return Ok(()); };
    let live = folder(game, rules)?;
    let global = store(game, "global")?;
    if let Some(id) = leaving {
        copy_settings(rules, &live, &store(game, id)?)?;
        if global.is_dir() { copy_settings(rules, &global, &live)?; }
    }
    if let Some(id) = entering {
        copy_settings(rules, &live, &global)?;
        let own = store(game, id)?;
        if own.is_dir() && fs::read_dir(&own).map_err(|e| e.to_string())?.next().is_some() { copy_settings(rules, &own, &live)?; }
    }
    Ok(())
}
/// Save file and its co-save (`.skse`, `.nvse`…), as the Vortex transfer does.
fn with_co_save(rules: &SaveRules, file: &str) -> Vec<String> {
    let stem = file.strip_suffix(&rules.extension).unwrap_or(file);
    std::iter::once(file.to_string()).chain(rules.co_save.iter().map(|ext| format!("{stem}{ext}"))).collect()
}
fn valid_save(rules: &SaveRules, file: &str) -> Result<(), String> {
    if file.contains(['/', '\\']) || !super::nexus_modules::relative(file) || !file.to_ascii_lowercase().ends_with(&rules.extension) { return Err("Save inválido.".into()); }
    Ok(())
}
#[derive(serde::Serialize)]
pub struct SaveEntry { #[serde(flatten)] info: SaveInfo, missing: Vec<String> }
#[derive(serde::Serialize)]
pub struct OtherSaves { id: Option<String>, name: String, count: usize }
#[derive(serde::Serialize)]
pub struct SavesView { dir: String, own: bool, saves: Vec<SaveEntry>, errors: Vec<String>, others: Vec<OtherSaves> }
fn profile_saves(game: &NexusGame) -> Result<(Option<String>, Vec<(Option<String>, String)>), String> {
    let profiles = super::nexus_user_profiles::load(game)?;
    let active = profiles.profiles.iter().find(|p| p.id == profiles.active).filter(|p| p.own_saves).map(|p| p.id.clone());
    let mut others: Vec<(Option<String>, String)> = profiles.profiles.iter().filter(|p| p.own_saves).map(|p| (Some(p.id.clone()), p.name.clone())).collect();
    others.insert(0, (None, "Saves compartilhados".into()));
    Ok((active, others))
}
fn list(dir: &Path, rules: &SaveRules) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir).into_iter().flatten().filter_map(Result::ok).map(|entry| entry.path())
        .filter(|path| path.is_file() && path.file_name().is_some_and(|name| name.to_string_lossy().to_ascii_lowercase().ends_with(&rules.extension))).collect();
    files.sort();
    files
}
pub(super) fn view(game: &NexusGame) -> Result<SavesView, String> {
    let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
    let rules = rules(&module)?;
    let (active, others) = profile_saves(game)?;
    let dir = saves_dir(game, rules, active.as_deref())?;
    // Active plugins of the current load order (plugins page), to name the missing ones.
    let active_plugins: Vec<String> = super::nexus_plugins::active(game).unwrap_or_default();
    let (mut saves, mut errors) = (Vec::new(), Vec::new());
    for path in list(&dir, rules) {
        match parse(&path, false) {
            Ok((info, _)) => {
                let missing = info.plugins.iter().filter(|plugin| !active_plugins.iter().any(|active| active.eq_ignore_ascii_case(plugin))).cloned().collect();
                saves.push(SaveEntry { info, missing });
            }
            Err(error) => errors.push(format!("{}: {error}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())),
        }
    }
    saves.sort_by(|a, b| b.info.created.cmp(&a.info.created));
    let others = others.into_iter().filter(|(id, _)| *id != active).map(|(id, name)| {
        let count = saves_dir(game, rules, id.as_deref()).map(|dir| list(&dir, rules).len()).unwrap_or(0);
        OtherSaves { id, name, count }
    }).collect();
    Ok(SavesView { dir: dir.to_string_lossy().into_owned(), own: active.is_some(), saves, errors, others })
}
fn with_game<T>(game_id: &str, change: bool, action: impl FnOnce(&NexusGame) -> Result<T, String>) -> Result<T, String> {
    let _lock = super::nexus_local::STORAGE.lock().map_err(|e| e.to_string())?;
    let data = super::nexus_local::load()?;
    let game = data.games.iter().find(|game| game.id == game_id).ok_or("Jogo não encontrado.")?;
    if change { super::nexus_deployment::idle(game)?; }
    action(game)
}
#[tauri::command]
pub async fn nexus_saves(game_id: String) -> Result<SavesView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, view)).await.map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn nexus_save_screenshot(game_id: String, file: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let rules = rules(&module)?;
        valid_save(rules, &file)?;
        let (active, _) = profile_saves(game)?;
        let (_, shot) = parse(&saves_dir(game, rules, active.as_deref())?.join(&file), true)?;
        shot.filter(|shot| shot.width > 0 && shot.height > 0).map(|shot| png_url(&shot)).unwrap_or_else(|| Ok(String::new()))
    })).await.map_err(|e| e.to_string())?
}
/// Deletes saves (and co-saves); a copy is kept in the game's PeliGames folder.
#[tauri::command]
pub async fn delete_nexus_saves(game_id: String, files: Vec<String>) -> Result<SavesView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let rules = rules(&module)?;
        let (active, _) = profile_saves(game)?;
        let dir = saves_dir(game, rules, active.as_deref())?;
        let kept = super::nexus_profile::directory(game)?.join("deleted-saves").join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&kept).map_err(|e| e.to_string())?;
        for file in &files {
            valid_save(rules, file)?;
            for name in with_co_save(rules, file) {
                let source = dir.join(&name);
                if !source.is_file() { continue; }
                fs::copy(&source, kept.join(&name)).map_err(|e| e.to_string())?;
                fs::remove_file(&source).map_err(|e| e.to_string())?;
            }
        }
        view(game)
    })).await.map_err(|e| e.to_string())?
}
/// Copies or moves saves of another profile (or the shared folder) into the
/// current one; existing saves are never overwritten.
#[tauri::command]
pub async fn transfer_nexus_saves(game_id: String, from: Option<String>, files: Vec<String>, keep: bool) -> Result<SavesView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let rules = rules(&module)?;
        let (active, others) = profile_saves(game)?;
        if from == active || !others.iter().any(|(id, _)| *id == from) { return Err("Perfil de origem inválido.".into()); }
        let (source, target) = (saves_dir(game, rules, from.as_deref())?, saves_dir(game, rules, active.as_deref())?);
        fs::create_dir_all(&target).map_err(|e| e.to_string())?;
        let mut skipped = Vec::new();
        for file in &files {
            valid_save(rules, file)?;
            if target.join(file).exists() { skipped.push(file.clone()); continue; }
            for name in with_co_save(rules, file) {
                if !source.join(&name).is_file() { continue; }
                fs::copy(source.join(&name), target.join(&name)).map_err(|e| e.to_string())?;
                if !keep { fs::remove_file(source.join(&name)).map_err(|e| e.to_string())?; }
            }
        }
        if !skipped.is_empty() { return Err(format!("Já existem no perfil atual, não foram trocados: {}", skipped.join(", "))); }
        view(game)
    })).await.map_err(|e| e.to_string())?
}
/// Lists the saves of another profile, for the transfer dialog.
#[tauri::command]
pub async fn nexus_other_saves(game_id: String, from: Option<String>) -> Result<Vec<SaveInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, false, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let rules = rules(&module)?;
        let mut saves: Vec<SaveInfo> = list(&saves_dir(game, rules, from.as_deref())?, rules).iter().filter_map(|path| parse(path, false).ok().map(|(info, _)| info)).collect();
        saves.sort_by(|a, b| b.created.cmp(&a.created));
        Ok(saves)
    })).await.map_err(|e| e.to_string())?
}
/// Applies the plugin list of a save (Vortex "Restore Save Game Plugins").
#[tauri::command]
pub async fn restore_nexus_save_plugins(game_id: String, file: String) -> Result<SavesView, String> {
    tauri::async_runtime::spawn_blocking(move || with_game(&game_id, true, |game| {
        let module = super::nexus_deployment::definition(game).ok_or("Módulo do jogo não identificado.")?;
        let rules = rules(&module)?;
        valid_save(rules, &file)?;
        let (active, _) = profile_saves(game)?;
        let (info, _) = parse(&saves_dir(game, rules, active.as_deref())?.join(&file), false)?;
        super::nexus_plugins::restore(game, &info.plugins)?;
        view(game)
    })).await.map_err(|e| e.to_string())?
}
#[cfg(test)]
mod tests {
    use super::*;
    fn wstring(out: &mut Vec<u8>, text: &str) { out.extend_from_slice(&(text.len() as u16).to_le_bytes()); out.extend_from_slice(text.as_bytes()); }
    #[test]
    fn skyrim_se_header_reads_the_compressed_plugin_list() {
        let mut body = vec![0x4e];
        body.extend_from_slice(&0u32.to_le_bytes());
        body.push(2);
        wstring(&mut body, "Skyrim.esm"); wstring(&mut body, "Mod.esp");
        body.extend_from_slice(&1u16.to_le_bytes());
        wstring(&mut body, "Light.esl");
        body.extend_from_slice(&[0; 64]);
        // LZ4 block of literals only.
        let mut lz4 = vec![0xF0];
        let mut rest = body.len() - 15;
        while rest >= 255 { lz4.push(255); rest -= 255; }
        lz4.push(rest as u8);
        lz4.extend_from_slice(&body);
        let mut save = b"TESV_SAVEGAME".to_vec();
        save.extend_from_slice(&0u32.to_le_bytes());
        save.extend_from_slice(&12u32.to_le_bytes());
        save.extend_from_slice(&7u32.to_le_bytes());
        wstring(&mut save, "Dovahkiin"); save.extend_from_slice(&42u32.to_le_bytes());
        wstring(&mut save, "Whiterun"); wstring(&mut save, "010.12.33"); wstring(&mut save, "Nord");
        save.extend_from_slice(&[0; 10]);
        save.extend_from_slice(&(133_000_000_000_000_000i64).to_le_bytes());
        save.extend_from_slice(&2u32.to_le_bytes()); save.extend_from_slice(&1u32.to_le_bytes());
        save.extend_from_slice(&2u16.to_le_bytes());
        save.extend_from_slice(&[10, 20, 30, 255, 40, 50, 60, 255]);
        save.extend_from_slice(&(body.len() as u32).to_le_bytes());
        save.extend_from_slice(&(lz4.len() as u32).to_le_bytes());
        save.extend_from_slice(&lz4);
        let path = std::env::temp_dir().join(format!("peligames-save-{}.ess", uuid::Uuid::new_v4()));
        fs::write(&path, &save).unwrap();
        let (info, shot) = parse(&path, true).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!((info.character.as_str(), info.level, info.location.as_str(), info.number), ("Dovahkiin", 42, "Whiterun", 7));
        assert_eq!(info.plugins, ["Skyrim.esm", "Mod.esp", "Light.esl"]);
        assert_eq!(info.created, 13_300_000_000 - 11_644_473_600);
        let shot = shot.unwrap();
        assert_eq!((shot.width, shot.height, shot.rgba.len()), (2, 1, 8));
        assert!(png_url(&shot).unwrap().starts_with("data:image/png;base64,iVBORw0KGgo"));
    }
    #[test]
    fn new_vegas_header_with_field_separators() {
        let mut save = b"FO3SAVEGAME".to_vec();
        save.extend_from_slice(&0u32.to_le_bytes()); save.extend_from_slice(&0x30u32.to_le_bytes()); save.push(b'|');
        // New Vegas: a 5-byte field (not 4) before the separator.
        save.extend_from_slice(&[1, 2, 3, 4, 5, b'|']);
        let field = |out: &mut Vec<u8>, bytes: &[u8]| { out.extend_from_slice(bytes); out.push(b'|'); };
        field(&mut save, &1u32.to_le_bytes()); field(&mut save, &1u32.to_le_bytes()); field(&mut save, &3u32.to_le_bytes());
        // An empty string has only the separator after its length, as the reader expects.
        let text = |out: &mut Vec<u8>, value: &str| { out.extend_from_slice(&(value.len() as u16).to_le_bytes()); out.push(b'|'); if !value.is_empty() { out.extend_from_slice(value.as_bytes()); out.push(b'|'); } };
        text(&mut save, "Courier"); text(&mut save, ""); field(&mut save, &12u32.to_le_bytes()); text(&mut save, "Goodsprings"); text(&mut save, "002.10.05");
        save.extend_from_slice(&[9, 9, 9]);
        save.extend_from_slice(&[0, 0, 0, 0, 0]);
        save.push(1); save.push(b'|');
        text(&mut save, "FalloutNV.esm");
        let path = std::env::temp_dir().join(format!("peligames-save-{}.fos", uuid::Uuid::new_v4()));
        fs::write(&path, &save).unwrap();
        let (info, _) = parse(&path, false).unwrap();
        fs::remove_file(&path).unwrap();
        assert_eq!((info.character.as_str(), info.level, info.location.as_str(), info.playtime.as_str()), ("Courier", 12, "Goodsprings", "002.10.05"));
        assert_eq!(info.plugins, ["FalloutNV.esm"]);
    }
    #[test]
    #[ignore = "Requires an isolated XDG_CONFIG_HOME (/tmp/peligames-*)"]
    fn profiles_switch_save_folder_and_game_settings() {
        use super::super::nexus_user_profiles as profiles;
        let config_root = PathBuf::from(std::env::var("XDG_CONFIG_HOME").expect("Isolated config required"));
        assert!(config_root.starts_with("/tmp") && config_root.file_name().unwrap().to_string_lossy().starts_with("peligames-"));
        let base = config_root.join(format!("nv-saves-{}", uuid::Uuid::new_v4()));
        let root = base.join("game");
        fs::create_dir_all(root.join("Data")).unwrap();
        fs::write(root.join("FalloutNV.exe"), b"MZ fixture").unwrap();
        let documents = base.join("prefix/pfx/drive_c/users/steamuser/Documents/My Games/FalloutNV");
        fs::create_dir_all(&documents).unwrap();
        fs::write(base.join("prefix/pfx/system.reg"), "").unwrap();
        fs::write(documents.join("Fallout.ini"), "[General]\r\nsLanguage=ENGLISH\r\n").unwrap();
        fs::write(documents.join("FalloutPrefs.ini"), "global prefs").unwrap();
        let game: NexusGame = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4().to_string(), "game": {"directory": root, "name": "Fallout New Vegas", "path": root.join("FalloutNV.exe")},
            "platform": "proton", "compat_data": base.join("prefix"), "mods": []
        })).unwrap();
        let mut data = super::super::nexus_local::Workspace { games: vec![game.clone()] };
        super::super::nexus_local::save(&data).unwrap();
        let mut list = profiles::load(&game).unwrap();
        let own = uuid::Uuid::new_v4().to_string();
        list.profiles.push(profiles::Profile { id: own.clone(), name: "Own".into(), order: Vec::new(), enabled: Vec::new(), own_saves: true, own_settings: true });
        let path = super::super::nexus_profile::directory(&game).unwrap().join("profiles.json");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, serde_json::to_vec(&list).unwrap()).unwrap();

        profiles::switch(&mut data, &game.id, &own).unwrap();
        let ini = fs::read_to_string(documents.join("Fallout.ini")).unwrap();
        assert!(ini.contains(&format!("SLocalSavePath=Saves\\{own}\\")) && ini.contains("sLanguage=ENGLISH"), "{ini}");
        assert!(documents.join("Saves").join(&own).is_dir());
        assert_eq!(fs::read_to_string(documents.join("FalloutPrefs.ini")).unwrap(), "global prefs", "perfil novo começa das configurações globais");
        fs::write(documents.join("FalloutPrefs.ini"), "own prefs").unwrap();
        // A save in the profile folder is listed.
        let mut save = b"FO3SAVEGAME".to_vec();
        save.extend_from_slice(&[0, 0, 0, 0, 0x30, 0, 0, 0, b'|', 1, 2, 3, 4, 5, b'|']);
        for value in [1u32, 1, 3] { save.extend_from_slice(&value.to_le_bytes()); save.push(b'|'); }
        for (index, value) in ["Courier", "", "", "Goodsprings", "001.00.00"].iter().enumerate() {
            if index == 2 { save.extend_from_slice(&5u32.to_le_bytes()); save.push(b'|'); continue; }
            save.extend_from_slice(&(value.len() as u16).to_le_bytes()); save.push(b'|');
            if !value.is_empty() { save.extend_from_slice(value.as_bytes()); save.push(b'|'); }
        }
        save.extend_from_slice(&[9, 9, 9, 0, 0, 0, 0, 0, 1, b'|']);
        save.extend_from_slice(&5u16.to_le_bytes()); save.push(b'|'); save.extend_from_slice(b"X.esm|");
        fs::write(documents.join("Saves").join(&own).join("Save 1.fos"), &save).unwrap();
        let page = view(&game).unwrap();
        assert!(page.own && page.saves.len() == 1 && page.saves[0].info.character == "Courier");
        assert_eq!(page.saves[0].info.plugins, ["X.esm"]);

        // Back to the shared profile: global INIs and shared saves.
        let mut data = super::super::nexus_local::load().unwrap();
        profiles::switch(&mut data, &game.id, profiles::DEFAULT_ID).unwrap();
        assert_eq!(fs::read_to_string(documents.join("FalloutPrefs.ini")).unwrap(), "global prefs");
        // The global Fallout.ini comes back as it was (no profile save folder in it).
        assert!(!fs::read_to_string(documents.join("Fallout.ini")).unwrap().contains(&own));
        assert!(!view(&game).unwrap().own);
        // And again: the profile's own INI returns.
        let mut data = super::super::nexus_local::load().unwrap();
        profiles::switch(&mut data, &game.id, &own).unwrap();
        assert_eq!(fs::read_to_string(documents.join("FalloutPrefs.ini")).unwrap(), "own prefs");
        fs::remove_dir_all(&base).unwrap();
    }
    #[test]
    fn save_path_values_follow_the_vortex_layout() {
        let rules: SaveRules = serde_json::from_value(serde_json::json!({"folder": "@documents/My Games/Enderal Special Edition/", "ini": "Enderal.ini", "base": "..\\Enderal Special Edition\\Saves", "extension": ".ess", "settings": []})).unwrap();
        assert_eq!(save_path_value(&rules, None), "..\\Enderal Special Edition\\Saves\\");
        assert_eq!(save_path_value(&rules, Some("abc")), "..\\Enderal Special Edition\\Saves\\abc\\");
        assert_eq!(with_co_save(&SaveRules { co_save: Some(".skse".into()), ..rules.clone() }, "Save 1.ess"), ["Save 1.ess", "Save 1.skse"]);
        assert!(valid_save(&rules, "../x.ess").is_err() && valid_save(&rules, "x.txt").is_err() && valid_save(&rules, "Quick.ess").is_ok());
    }
}
