//! Bounded Larian package metadata reader and modsettings merger (no executable tools).
use super::nexus_local::NexusGame;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
fn lz4(input: &[u8], limit: usize) -> Result<Vec<u8>, String> { lz4_with(input, limit, false) }
/// The first `want` bytes of an LZ4 block; the rest of the input is not needed.
pub(super) fn lz4_prefix(input: &[u8], want: usize) -> Result<Vec<u8>, String> { lz4_with(input, want, true) }
fn lz4_with(input: &[u8], limit: usize, prefix: bool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut i = 0;
    fn count(input: &[u8], i: &mut usize, base: usize) -> Result<usize, String> {
        let mut n = base;
        if base == 15 {
            loop {
                let b = *input.get(*i).ok_or("LZ4 truncado.")? as usize;
                *i += 1;
                n = n.checked_add(b).ok_or("LZ4 inválido.")?;
                if b != 255 {
                    break;
                }
            }
        }
        Ok(n)
    }
    while i < input.len() {
        let token = input[i];
        i += 1;
        let n = count(input, &mut i, (token >> 4) as usize)?;
        if prefix && n > limit.saturating_sub(out.len()) {
            out.extend_from_slice(&input[i..(i + limit - out.len()).min(input.len())]);
            return Ok(out);
        }
        if n > limit.saturating_sub(out.len()) || n > input.len() - i {
            return Err("Limite LZ4 excedido.".into());
        }
        out.extend_from_slice(&input[i..i + n]);
        i += n;
        if i == input.len() {
            break;
        }
        if i + 2 > input.len() {
            return Err("LZ4 truncado.".into());
        }
        let back = u16::from_le_bytes(input[i..i + 2].try_into().unwrap()) as usize;
        i += 2;
        let n = count(input, &mut i, (token & 15) as usize)?
            .checked_add(4)
            .ok_or("LZ4 inválido.")?;
        if back == 0 || back > out.len() {
            return Err("Referência LZ4 inválida.".into());
        }
        let n = if prefix { n.min(limit - out.len()) } else if n > limit - out.len() { return Err("Referência LZ4 inválida.".into()) } else { n };
        for _ in 0..n {
            out.push(out[out.len() - back]);
        }
        if prefix && out.len() >= limit { return Ok(out); }
    }
    Ok(out)
}
fn decompress(input: &[u8], flags: u32, limit: usize) -> Result<Vec<u8>, String> {
    if input.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
        // libarchive's runtime dependency supplies libzstd; don't run tools from a mod.
        #[cfg(unix)]
        unsafe {
            let handle = libc::dlopen(c"libzstd.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if handle.is_null() {
                return Err("Instale libzstd para ler este pacote BG3.".into());
            }
            let f = libc::dlsym(handle, c"ZSTD_decompress".as_ptr());
            let check = libc::dlsym(handle, c"ZSTD_isError".as_ptr());
            if f.is_null() || check.is_null() {
                libc::dlclose(handle);
                return Err("Biblioteca zstd incompatível.".into());
            }
            let decode: unsafe extern "C" fn(
                *mut libc::c_void,
                usize,
                *const libc::c_void,
                usize,
            ) -> usize = std::mem::transmute(f);
            let is_error: unsafe extern "C" fn(usize) -> u32 = std::mem::transmute(check);
            let mut out = vec![0; limit];
            let n = decode(
                out.as_mut_ptr().cast(),
                limit,
                input.as_ptr().cast(),
                input.len(),
            );
            let bad = is_error(n) != 0;
            libc::dlclose(handle);
            if bad || n > limit {
                return Err("Conteúdo zstd inválido ou grande demais.".into());
            }
            out.truncate(n);
            return Ok(out);
        }
    }
    match flags & 15 {
        0 if input.len() <= limit => Ok(input.to_vec()),
        1 => {
            let mut out = Vec::new();
            flate2::read::ZlibDecoder::new(input)
                .take(limit as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|e| e.to_string())?;
            if out.len() > limit {
                return Err("Limite zlib excedido.".into());
            }
            Ok(out)
        }
        2 | 3 => lz4(input, limit),
        _ => Err("Compressão LSPK não suportada.".into()),
    }
}
pub(super) fn chunk(file: &mut fs::File, offset: u64, size: usize, limit: usize) -> Result<Vec<u8>, String> {
    if size > limit
        || offset
            .checked_add(size as u64)
            .is_none_or(|end| end > file.metadata().map(|m| m.len()).unwrap_or(0))
    {
        return Err("Bloco PAK inválido ou grande demais.".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut out = vec![0; size];
    file.read_exact(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}
#[derive(Clone, Default)]
struct Xml {
    name: String,
    attrs: BTreeMap<String, String>,
    children: Vec<Xml>,
}
fn parse(input: &str) -> Result<Xml, String> {
    use quick_xml::events::Event;
    if input.len() > 2 * 1024 * 1024 {
        return Err("XML BG3 grande demais.".into());
    }
    let mut reader = quick_xml::Reader::from_str(input);
    let mut stack = vec![Xml::default()];
    let mut nodes = 0;
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let empty = matches!(&event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                let name =
                    String::from_utf8(e.name().as_ref().to_vec()).map_err(|e| e.to_string())?;
                let attrs = e
                    .attributes()
                    .map(|a| {
                        let a = a.map_err(|e| e.to_string())?;
                        Ok((
                            String::from_utf8(a.key.as_ref().to_vec())
                                .map_err(|e| e.to_string())?,
                            a.decoded_and_normalized_value(
                                quick_xml::XmlVersion::Implicit1_0,
                                reader.decoder(),
                            )
                            .map_err(|e| e.to_string())?
                            .into_owned(),
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, String>>()?;
                nodes += 1;
                if nodes > 20000 || stack.len() > 64 {
                    return Err("XML BG3 complexo demais.".into());
                }
                let node = Xml {
                    name,
                    attrs,
                    children: vec![],
                };
                if empty {
                    stack.last_mut().unwrap().children.push(node)
                } else {
                    stack.push(node)
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err("XML BG3 inválido.".into());
                }
                let node = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(node);
            }
            Event::DocType(_) => return Err("DTD não permitido em mods BG3.".into()),
            Event::Eof => break,
            Event::Text(e) if !e.decode().map_err(|e| e.to_string())?.trim().is_empty() => {
                return Err("Texto XML BG3 inesperado.".into())
            }
            _ => {}
        }
    }
    if stack.len() != 1 || stack[0].children.len() != 1 {
        return Err("XML BG3 inválido.".into());
    }
    Ok(stack.pop().unwrap().children.remove(0))
}
impl Xml {
    fn id(&self) -> &str {
        self.attrs.get("id").map(String::as_str).unwrap_or("")
    }
    fn value(&self, id: &str) -> Option<&str> {
        self.children
            .iter()
            .find(|n| n.name == "attribute" && n.id() == id)?
            .attrs
            .get("value")
            .map(String::as_str)
    }
    fn find(&mut self, id: &str) -> Option<&mut Self> {
        if self.name == "node" && self.id() == id {
            return Some(self);
        }
        for child in &mut self.children {
            if let Some(found) = child.find(id) {
                return Some(found);
            }
        }
        None
    }
    fn render(&self) -> String {
        let attrs = self
            .attrs
            .iter()
            .map(|(k, v)| format!(" {k}=\"{}\"", quick_xml::escape::escape(v)))
            .collect::<String>();
        if self.children.is_empty() {
            format!("<{}{attrs}/>", self.name)
        } else {
            format!(
                "<{}{attrs}>{}</{}>",
                self.name,
                self.children.iter().map(Self::render).collect::<String>(),
                self.name
            )
        }
    }
    fn children(&mut self) -> &mut Vec<Self> {
        if !self.children.iter().any(|n| n.name == "children") {
            self.children.push(Xml {
                name: "children".into(),
                ..Default::default()
            })
        }
        &mut self
            .children
            .iter_mut()
            .find(|n| n.name == "children")
            .unwrap()
            .children
    }
}
fn attr(id: &str, kind: &str, value: &str) -> Xml {
    Xml {
        name: "attribute".into(),
        attrs: BTreeMap::from([
            ("id".into(), id.into()),
            ("type".into(), kind.into()),
            ("value".into(), value.into()),
        ]),
        children: vec![],
    }
}
fn node(id: &str, attrs: Vec<Xml>) -> Xml {
    Xml {
        name: "node".into(),
        attrs: BTreeMap::from([("id".into(), id.into())]),
        children: attrs,
    }
}
#[derive(Clone)]
pub(super) struct Info {
    pub(super) uuid: String,
    folder: String,
    pub(super) name: String,
    version: String,
    md5: String,
    /// Mods listed under `Dependencies` in meta.lsx, as (UUID, name); the
    /// game's own modules (Gustav, Shared) are left out.
    pub(super) dependencies: Vec<(String, String)>,
}
pub(super) fn metadata(path: &Path) -> Result<Vec<Info>, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let header = chunk(&mut file, 0, 16, 16)?;
    if &header[..4] != b"LSPK" {
        return Err("O arquivo .pak não é um pacote Larian LSPK.".into());
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    let entry_size = match version {
        15 | 16 => 296,
        18 => 272,
        _ => return Err(format!("LSPK versão {version} não suportada.")),
    };
    let offset = u64::from_le_bytes(header[8..16].try_into().unwrap());
    let list_header = chunk(&mut file, offset, 8, 8)?;
    let count = u32::from_le_bytes(list_header[..4].try_into().unwrap()) as usize;
    let size = u32::from_le_bytes(list_header[4..].try_into().unwrap()) as usize;
    if count > 100000 {
        return Err("Lista LSPK grande demais.".into());
    }
    let list = lz4(
        &chunk(&mut file, offset + 8, size, 32 * 1024 * 1024)?,
        count * entry_size,
    )?;
    if list.len() != count * entry_size {
        return Err("Lista LSPK truncada.".into());
    }
    let mut result = Vec::new();
    for entry in list.chunks_exact(entry_size) {
        let end = entry[..256].iter().position(|b| *b == 0).unwrap_or(256);
        let name = String::from_utf8(entry[..end].to_vec())
            .map_err(|e| e.to_string())?
            .replace('\\', "/");
        let pieces = name.split('/').collect::<Vec<_>>();
        if pieces.len() != 3
            || !pieces[0].eq_ignore_ascii_case("Mods")
            || !pieces[2].eq_ignore_ascii_case("meta.lsx")
        {
            continue;
        }
        let (part, flags, start, size) = if version == 18 {
            (
                entry[262] as u32,
                entry[263] as u32,
                u32::from_le_bytes(entry[256..260].try_into().unwrap()) as u64
                    | ((u16::from_le_bytes(entry[260..262].try_into().unwrap()) as u64) << 32),
                u32::from_le_bytes(entry[264..268].try_into().unwrap()) as usize,
            )
        } else {
            (
                u32::from_le_bytes(entry[280..284].try_into().unwrap()),
                u32::from_le_bytes(entry[284..288].try_into().unwrap()),
                u64::from_le_bytes(entry[256..264].try_into().unwrap()),
                usize::try_from(u64::from_le_bytes(entry[264..272].try_into().unwrap()))
                    .map_err(|_| "Tamanho LSPK inválido.")?,
            )
        };
        // Split packages keep later data in `Name_1.pak`, `Name_2.pak`, ... beside the first file.
        let mut sibling;
        let source = if part == 0 { &mut file } else {
            let stem = path.file_stem().and_then(|s| s.to_str()).ok_or("Nome de pacote inválido.")?;
            let next = path.with_file_name(format!("{stem}_{part}.pak"));
            sibling = fs::File::open(&next).map_err(|_| format!("Parte {part} do pacote ausente: {stem}_{part}.pak"))?;
            &mut sibling
        };
        let bytes = decompress(
            &chunk(source, start, size, 2 * 1024 * 1024)?,
            flags,
            2 * 1024 * 1024,
        )?;
        let xml = String::from_utf8(bytes).map_err(|_| "Metadados BG3 não são UTF-8.")?;
        let mut tree = parse(xml.trim_start_matches('\u{feff}'))?;
        let dependencies: Vec<(String, String)> = tree.find("Dependencies").map(|deps| deps.children().iter()
            .filter_map(|n| {
                let name = n.value("Name").or_else(|| n.value("Folder")).unwrap_or("").to_string();
                let builtin = ["Gustav", "Shared"].iter().any(|base| name.starts_with(base));
                n.value("UUID").filter(|_| !builtin).map(|uuid| (uuid.to_string(), name))
            }).collect()).unwrap_or_default();
        let info = tree
            .find("ModuleInfo")
            .ok_or("ModuleInfo ausente no PAK.")?;
        let uuid = info.value("UUID").ok_or("UUID BG3 ausente.")?.to_string();
        uuid::Uuid::parse_str(&uuid).map_err(|_| "UUID BG3 inválido.")?;
        if ["Gustav", "GustavDev", "GustavX", "Shared", "SharedDev"].contains(&pieces[1]) {
            continue;
        }
        let folder = info
            .value("Folder")
            .ok_or("Pasta do módulo BG3 ausente.")?
            .to_string();
        if !super::nexus_modules::relative(&folder) || folder.contains('/') {
            return Err("Pasta de módulo BG3 inválida.".into());
        }
        result.push(Info {
            uuid,
            folder,
            name: info.value("Name").unwrap_or(pieces[1]).into(),
            version: info
                .value("Version64")
                .or_else(|| info.value("Version"))
                .unwrap_or("0")
                .into(),
            md5: info.value("MD5").unwrap_or("").into(),
            dependencies,
        });
    }
    Ok(result)
}
/// modsettings.lsx layout of the installed game, from `bin/bg3.exe`'s file
/// version as in the Vortex extension (patch 7: 4.58.49, patch 8: 4.67.58).
#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Format { V6, V7, V8 }
pub(super) fn format(game_root: &Path) -> Format {
    match file_version(&game_root.join("bin/bg3.exe")) {
        Some(version) if version >= [4, 67, 58] => Format::V8,
        Some(version) if version >= [4, 58, 49] => Format::V7,
        Some(_) => Format::V6,
        // Unknown builds are treated as current, like Vortex.
        None => Format::V8,
    }
}
/// `VS_FIXEDFILEINFO` file version (major, minor, build) from the PE resources.
pub(super) fn file_version(path: &Path) -> Option<[u16; 3]> {
    let mut file = fs::File::open(path).ok()?;
    let dos = chunk(&mut file, 0, 64, 64).ok()?;
    if &dos[..2] != b"MZ" { return None; }
    let pe = u32::from_le_bytes(dos[60..64].try_into().ok()?) as u64;
    let head = chunk(&mut file, pe, 24, 24).ok()?;
    if &head[..4] != b"PE\0\0" { return None; }
    let sections = u16::from_le_bytes(head[6..8].try_into().ok()?) as usize;
    let optional = u16::from_le_bytes(head[20..22].try_into().ok()?) as u64;
    let table = chunk(&mut file, pe + 24 + optional, sections * 40, 96 * 40).ok()?;
    let rsrc = table.chunks_exact(40).find(|section| section.starts_with(b".rsrc"))?;
    let size = u32::from_le_bytes(rsrc[16..20].try_into().ok()?) as usize;
    let offset = u32::from_le_bytes(rsrc[20..24].try_into().ok()?) as u64;
    let data = chunk(&mut file, offset, size, 64 << 20).ok()?;
    let at = data.windows(4).position(|w| w == [0xBD, 0x04, 0xEF, 0xFE])?;
    let ms = u32::from_le_bytes(data.get(at + 8..at + 12)?.try_into().ok()?);
    let ls = u32::from_le_bytes(data.get(at + 12..at + 16)?.try_into().ok()?);
    Some([(ms >> 16) as u16, ms as u16, (ls >> 16) as u16])
}
/// File the game would create on first run, as written by the Vortex extension
/// when modsettings.lsx is missing.
pub(super) fn default_settings(format: Format) -> &'static str {
    match format {
        Format::V8 => r#"<?xml version="1.0" encoding="UTF-8"?>
<save>
    <version major="4" minor="8" revision="0" build="10"/>
    <region id="ModuleSettings">
        <node id="root">
            <children>
                <node id="Mods">
                    <children>
                        <node id="ModuleShortDesc">
                            <attribute id="Folder" type="LSString" value="GustavX"/>
                            <attribute id="MD5" type="LSString" value=""/>
                            <attribute id="Name" type="LSString" value="GustavX"/>
                            <attribute id="PublishHandle" type="uint64" value="0"/>
                            <attribute id="UUID" type="guid" value="cb555efe-2d9e-131f-8195-a89329d218ea"/>
                            <attribute id="Version64" type="int64" value="36028797018963968"/>
                        </node>
                    </children>
                </node>
            </children>
        </node>
    </region>
</save>"#,
        Format::V7 => r#"<?xml version="1.0" encoding="UTF-8"?>
<save>
  <version major="4" minor="7" revision="1" build="200"/>
  <region id="ModuleSettings">
    <node id="root">
      <children>
        <node id="Mods">
          <children>
            <node id="ModuleShortDesc">
              <attribute id="Folder" type="LSString" value="GustavDev"/>
              <attribute id="MD5" type="LSString" value=""/>
              <attribute id="Name" type="LSString" value="GustavDev"/>
              <attribute id="PublishHandle" type="uint64" value="0"/>
              <attribute id="UUID" type="guid" value="28ac9ce2-2aba-8cda-b3b5-6e922f71b6b8"/>
              <attribute id="Version64" type="int64" value="36028797018963968"/>
            </node>
          </children>
        </node>
      </children>
    </node>
  </region>
</save>"#,
        Format::V6 => r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<save>
  <version major="4" minor="0" revision="10" build="100"/>
  <region id="ModuleSettings">
    <node id="root">
      <children>
        <node id="ModOrder">
          <children/>
        </node>
        <node id="Mods">
          <children>
            <node id="ModuleShortDesc">
              <attribute id="Folder" type="LSWString" value="GustavDev"/>
              <attribute id="MD5" type="LSString" value=""/>
              <attribute id="Name" type="FixedString" value="GustavDev"/>
              <attribute id="UUID" type="FixedString" value="28ac9ce2-2aba-8cda-b3b5-6e922f71b6b8"/>
              <attribute id="Version64" type="int64" value="36028797018963968"/>
            </node>
          </children>
        </node>
      </children>
    </node>
  </region>
</save>"#,
    }
}
/// Load order kept by PeliGames, like the Vortex `loadOrder.json`: pak file
/// names in order, and whether each change is also written to the game.
#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Store {
    #[serde(default)]
    pub(super) order: Vec<String>,
    #[serde(default = "enabled")]
    pub(super) auto_export: bool,
}
fn enabled() -> bool { true }
impl Default for Store {
    fn default() -> Self { Store { order: Vec::new(), auto_export: true } }
}
impl Store {
    fn path(game: &NexusGame) -> Result<std::path::PathBuf, String> {
        Ok(super::nexus_profile::directory(game)?.join("bg3-load-order.json"))
    }
    pub(super) fn load(game: &NexusGame) -> Result<Self, String> {
        match fs::read(Self::path(game)?) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Ordem de carga BG3 salva inválida.".into()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.to_string()),
        }
    }
    pub(super) fn save(&self, game: &NexusGame) -> Result<(), String> {
        let path = Self::path(game)?;
        fs::create_dir_all(path.parent().ok_or("Pasta inválida.")?).map_err(|e| e.to_string())?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        fs::write(&temporary, serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        fs::rename(&temporary, &path).map_err(|e| { let _ = fs::remove_file(&temporary); e.to_string() })
    }
}
/// The game's Mods folder inside the prefix.
pub(super) fn live_mods(game: &NexusGame, data: &str) -> Result<std::path::PathBuf, String> {
    let probe = super::nexus_deployment::destination(game, &format!("{data}/probe.pak"))?;
    Ok(probe.parent().ok_or("Pasta Mods inválida.")?.to_path_buf())
}
/// `.pak` files of a folder, by name.
pub(super) fn paks_in(dir: &Path) -> Result<Vec<(String, std::path::PathBuf)>, String> {
    if !dir.is_dir() { return Ok(Vec::new()); }
    let mut paks: Vec<_> = fs::read_dir(dir).map_err(|e| e.to_string())?.filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_file() || t.is_symlink())
            && entry.path().extension().is_some_and(|ext| ext.eq_ignore_ascii_case("pak")))
        .map(|entry| (entry.file_name().to_string_lossy().into_owned(), entry.path())).collect();
    // `Name_1.pak` beside `Name.pak` is a part of that package, not another mod.
    let names: BTreeSet<String> = paks.iter().map(|(name, _)| name.to_ascii_lowercase()).collect();
    paks.retain(|(name, _)| !part_of(name).is_some_and(|first| names.contains(&first)));
    paks.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(paks)
}
/// `Name_3.pak` → `name.pak` (lower case), the first file of a split package.
fn part_of(name: &str) -> Option<String> {
    let stem = name.get(..name.len().checked_sub(4)?)?;
    let (base, number) = stem.rsplit_once('_')?;
    (!base.is_empty() && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())).then(|| format!("{}.pak", base.to_ascii_lowercase()))
}
/// Known files take their saved position; new ones go to the end in the given
/// order, as Vortex appends newly found paks to the bottom.
pub(super) fn arrange<T>(order: &[String], items: Vec<(String, T)>) -> Vec<(String, T)> {
    let position = |name: &str| order.iter().position(|known| known.eq_ignore_ascii_case(name)).unwrap_or(usize::MAX);
    let mut items: Vec<_> = items.into_iter().enumerate().collect();
    items.sort_by_key(|(index, (name, _))| (position(name), *index));
    items.into_iter().map(|(_, item)| item).collect()
}
/// Pak files in load order with their metadata. Managed paks come from
/// `staged` (the profile being deployed); other paks in the live Mods folder,
/// placed by the player or the game's mod manager, stay in the order as in Vortex.
fn load_order(game: &NexusGame, data: &str, staged: Option<(&Path, &BTreeSet<String>)>) -> Result<(Store, Vec<(String, Vec<Info>)>), String> {
    let mut priority: BTreeMap<String, usize> = BTreeMap::new();
    for (index, m) in game.mods.iter().filter(|m| m.enabled).enumerate() {
        for (_, target) in m.installed_files.iter().chain(m.fomod_files.iter()).flatten() {
            if let Some(name) = target.strip_prefix(&format!("{data}/")).filter(|n| !n.contains('/')) {
                priority.insert(name.to_ascii_lowercase(), index);
            }
        }
    }
    let mut files: Vec<(String, std::path::PathBuf, bool)> = Vec::new();
    if let Some((runtime, _)) = staged {
        let mut own = paks_in(&runtime.join(data))?;
        own.sort_by_key(|(name, _)| (priority.get(&name.to_ascii_lowercase()).copied().unwrap_or(usize::MAX), name.clone()));
        files.extend(own.into_iter().map(|(name, path)| (name, path, true)));
    }
    for (name, path) in paks_in(&live_mods(game, data)?)? {
        let lower = name.to_ascii_lowercase();
        let replaced = staged.is_some_and(|(_, owned)| owned.contains(&lower));
        if !replaced && !files.iter().any(|(other, _, _)| other.eq_ignore_ascii_case(&name)) {
            files.push((name, path, false));
        }
    }
    let mut store = Store::load(game)?;
    let files = arrange(&store.order, files.into_iter().map(|(name, path, own)| (name, (path, own))).collect());
    store.order = files.iter().map(|(name, _)| name.clone()).collect();
    let mut result = Vec::new();
    for (name, (path, own)) in files {
        // A broken external pak is skipped, as Vortex does; a managed one stops the deployment.
        let infos = match metadata(&path) { Ok(infos) => infos, Err(error) if own => return Err(format!("{name}: {error}")), Err(_) => Vec::new() };
        result.push((name, infos));
    }
    Ok((store, result))
}
pub(super) fn activation(
    game: &NexusGame,
    runtime: &Path,
    data: &str,
    original: &str,
    old: &BTreeMap<String, String>,
) -> Result<String, String> {
    let owned: BTreeSet<String> = old.keys().filter_map(|p| p.strip_prefix(&format!("{data}/")))
        .filter(|name| !name.contains('/')).map(|name| name.to_ascii_lowercase()).collect();
    let (store, paks) = load_order(game, data, Some((runtime, &owned)))?;
    store.save(game)?;
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let base = if original.trim().is_empty() { default_settings(format(root)).to_string() } else { original.to_string() };
    // With automatic export off, the game's load order changes only on "Export to game".
    if !store.auto_export { return Ok(base); }
    merge(&base, &paks.into_iter().flat_map(|(_, infos)| infos).collect::<Vec<_>>())
}
/// Rebuilds the module list as the Vortex export does: the game's own entries
/// (Gustav*) stay, followed by the load order. Files from patch 7 on have no
/// `ModOrder` node and take the order of `Mods`; older ones keep `ModOrder`.
pub(super) fn merge(original: &str, infos: &[Info]) -> Result<String, String> {
    let mut seen = BTreeSet::new();
    let infos: Vec<&Info> = infos.iter().filter(|info| seen.insert(info.uuid.to_ascii_lowercase())).collect();
    let mut tree = parse(original.trim_start_matches('\u{feff}'))?;
    let legacy = tree.find("ModOrder").is_some();
    let mods = tree.find("Mods").ok_or("modsettings.lsx existente não contém os nós esperados; preservado.")?.children();
    mods.retain(|n| n.value("Name").is_some_and(|name| name.starts_with("Gustav")));
    for info in &infos {
        let fields = if legacy {
            vec![
                attr("Folder", "LSString", &info.folder),
                attr("MD5", "LSString", &info.md5),
                attr("Name", "LSString", &info.name),
                attr("UUID", "FixedString", &info.uuid),
                attr("Version64", "int64", &info.version),
            ]
        } else {
            vec![
                attr("Folder", "LSString", &info.folder),
                attr("MD5", "LSString", &info.md5),
                attr("Name", "LSString", &info.name),
                attr("PublishHandle", "uint64", "0"),
                attr("UUID", "guid", &info.uuid),
                attr("Version64", "int64", &info.version),
            ]
        };
        mods.push(node("ModuleShortDesc", fields));
    }
    if legacy {
        let order = tree.find("ModOrder").ok_or("ModOrder ausente.")?.children();
        order.clear();
        order.extend(infos.iter().map(|info| node("Module", vec![attr("UUID", "FixedString", &info.uuid)])));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}\n",
        tree.render()
    ))
}
/// Module UUIDs in the order of a modsettings.lsx (game or file).
pub(super) fn lsx_order(xml: &str) -> Result<Vec<String>, String> {
    let mut tree = parse(xml.trim_start_matches('\u{feff}'))?;
    let key = if tree.find("ModOrder").is_some_and(|n| !n.children().is_empty()) { "ModOrder" } else { "Mods" };
    let list = tree.find(key).ok_or("O arquivo não contém uma lista de mods do BG3.")?.children();
    Ok(list.iter().filter(|n| !n.value("Name").is_some_and(|name| name.starts_with("Gustav")))
        .filter_map(|n| n.value("UUID").map(str::to_ascii_lowercase)).collect())
}
/// Module UUIDs of a BG3 Mod Manager export (a list, or an object holding the list).
pub(super) fn bg3mm_order(json: &str) -> Result<Vec<String>, String> {
    if json.len() > 2 * 1024 * 1024 { return Err("Arquivo do BG3 Mod Manager grande demais.".into()); }
    let value: serde_json::Value = serde_json::from_str(json.trim_start_matches('\u{feff}')).map_err(|_| "Arquivo do BG3 Mod Manager inválido.")?;
    let has_uuid = |list: &Vec<serde_json::Value>| list.iter().any(|entry| entry.get("UUID").is_some());
    let list = value.as_array().filter(|list| has_uuid(list))
        .or_else(|| value.as_object().and_then(|object| object.values().filter_map(|v| v.as_array()).find(|list| has_uuid(list))))
        .ok_or("O arquivo do BG3 Mod Manager não contém uma lista de mods.")?;
    Ok(list.iter().filter_map(|entry| entry.get("UUID")?.as_str().map(str::to_ascii_lowercase)).collect())
}
/// Pak names sorted by the first imported UUID they hold; the others keep
/// their order after them.
pub(super) fn reorder(paks: &[(String, Vec<Info>)], uuids: &[String]) -> Vec<String> {
    let rank = |infos: &Vec<Info>| infos.iter().filter_map(|info| uuids.iter().position(|uuid| uuid.eq_ignore_ascii_case(&info.uuid))).min().unwrap_or(usize::MAX);
    let mut ranked: Vec<_> = paks.iter().enumerate().map(|(index, (name, infos))| (rank(infos), index, name.clone())).collect();
    ranked.sort();
    ranked.into_iter().map(|(_, _, name)| name).collect()
}
#[derive(serde::Serialize)]
pub struct Entry {
    file: String,
    name: String,
    uuid: Option<String>,
    /// Pak without meta.lsx (override): always loaded, not written to the list.
    locked: bool,
    managed: Option<String>,
    missing: Vec<String>,
    late: Vec<String>,
}
#[derive(serde::Serialize)]
pub struct View {
    entries: Vec<Entry>,
    auto_export: bool,
    format: Format,
    settings: String,
    mod_fixer: bool,
}
/// Rule of the game's BG3 activation (load order file and Mods folder).
pub(super) fn rule(game: &NexusGame) -> Result<super::nexus_modules::Activation, String> {
    super::nexus_deployment::definition(game).and_then(|module| module.activation)
        .filter(|rule| rule.kind == super::nexus_modules::ActivationKind::BaldursGate3)
        .ok_or_else(|| "Este jogo não tem ordem de carga do Baldur's Gate 3.".into())
}
pub(super) fn view(game: &NexusGame) -> Result<View, String> {
    let rule = rule(game)?;
    let (store, paks) = load_order(game, &rule.data, None)?;
    let mut owners: BTreeMap<String, String> = BTreeMap::new();
    for m in game.mods.iter().filter(|m| m.enabled) {
        for (_, target) in m.installed_files.iter().chain(m.fomod_files.iter()).flatten() {
            if let Some(name) = target.strip_prefix(&format!("{}/", rule.data)) {
                owners.insert(name.to_ascii_lowercase(), m.nexus_source.as_ref().map(|s| s.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| m.name.clone()));
            }
        }
    }
    let position: BTreeMap<String, usize> = paks.iter().enumerate()
        .flat_map(|(index, (_, infos))| infos.iter().map(move |info| (info.uuid.to_ascii_lowercase(), index))).collect();
    let mut entries: Vec<Entry> = paks.iter().enumerate().map(|(index, (file, infos))| {
        let deps = infos.iter().flat_map(|info| info.dependencies.iter());
        let label = |(uuid, name): &(String, String)| if name.is_empty() { uuid.clone() } else { name.clone() };
        Entry {
            file: file.clone(),
            name: infos.first().map(|info| info.name.clone()).unwrap_or_else(|| file.trim_end_matches(".pak").to_string()),
            uuid: infos.first().map(|info| info.uuid.clone()),
            locked: infos.is_empty(),
            managed: owners.get(&file.to_ascii_lowercase()).cloned(),
            missing: deps.clone().filter(|(uuid, _)| !position.contains_key(&uuid.to_ascii_lowercase())).map(label).collect(),
            late: deps.filter(|(uuid, _)| position.get(&uuid.to_ascii_lowercase()).is_some_and(|at| *at > index)).map(label).collect(),
        }
    }).collect();
    // Locked entries are listed first, as on the Vortex load order page.
    entries.sort_by_key(|entry| !entry.locked);
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let format = format(root);
    Ok(View {
        mod_fixer: format == Format::V6 && !paks.iter().any(|(name, _)| name.eq_ignore_ascii_case("ModFixer.pak")),
        settings: super::nexus_deployment::destination(game, &rule.path)?.to_string_lossy().into_owned(),
        entries, auto_export: store.auto_export, format,
    })
}
/// The game's modsettings.lsx with the current load order (Vortex "Export").
pub(super) fn exported(game: &NexusGame) -> Result<String, String> {
    let rule = rule(game)?;
    let (_, paks) = load_order(game, &rule.data, None)?;
    let path = super::nexus_deployment::destination(game, &rule.path)?;
    let original = match fs::read_to_string(&path) { Ok(text) => text, Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(), Err(e) => return Err(e.to_string()) };
    let root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let base = if original.trim().is_empty() { default_settings(format(root)).to_string() } else { original };
    merge(&base, &paks.into_iter().flat_map(|(_, infos)| infos).collect::<Vec<_>>())
}
/// Pak names of the live Mods folder with their metadata, in the saved order.
pub(super) fn current(game: &NexusGame) -> Result<Vec<(String, Vec<Info>)>, String> {
    Ok(load_order(game, &rule(game)?.data, None)?.1)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn info(uuid: &str, deps: &[&str]) -> Info {
        Info { uuid: uuid.into(), folder: format!("F{uuid}"), name: format!("N{uuid}"), version: "1".into(), md5: String::new(), dependencies: deps.iter().map(|d| (d.to_string(), String::new())).collect() }
    }
    const V8: &str = r#"<?xml version="1.0" encoding="UTF-8"?><save><version major="4" minor="8" revision="0" build="10"/><region id="ModuleSettings"><node id="root"><children><node id="Mods"><children><node id="ModuleShortDesc"><attribute id="Folder" type="LSString" value="GustavX"/><attribute id="Name" type="LSString" value="GustavX"/><attribute id="UUID" type="guid" value="cb555efe-2d9e-131f-8195-a89329d218ea"/></node><node id="ModuleShortDesc"><attribute id="Name" type="LSString" value="Gone"/><attribute id="UUID" type="guid" value="gone"/></node></children></node></children></node></region></save>"#;
    #[test]
    fn saved_order_wins_and_new_paks_go_to_the_end() {
        let order = vec!["b.pak".to_string(), "A.pak".to_string()];
        let items = vec![("new2.pak".to_string(), ()), ("a.pak".to_string(), ()), ("new1.pak".to_string(), ()), ("b.pak".to_string(), ())];
        let names: Vec<_> = arrange(&order, items).into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["b.pak", "a.pak", "new2.pak", "new1.pak"]);
    }
    #[test]
    fn patch_8_settings_keep_game_entries_and_write_the_order() {
        let out = merge(V8, &[info("b", &[]), info("a", &[]), info("B", &[])]).unwrap();
        assert!(!out.contains("ModOrder"));
        // Entries without a pak in the load order are dropped, as in the Vortex export.
        assert!(out.contains(r#"value="GustavX""#) && !out.contains(r#"value="Gone""#));
        assert!(out.contains(r#"<attribute id="PublishHandle" type="uint64" value="0"/><attribute id="UUID" type="guid" value="b"/>"#));
        assert!(out.find(r#"value="Nb""#).unwrap() < out.find(r#"value="Na""#).unwrap());
        assert_eq!(out.matches(r#"value="Nb""#).count(), 1, "UUID repetido entra uma vez");
        assert_eq!(merge(&out, &[info("b", &[]), info("a", &[])]).unwrap(), out);
        assert_eq!(lsx_order(&out).unwrap(), ["b", "a"]);
    }
    #[test]
    fn patch_6_settings_keep_the_mod_order_node() {
        let out = merge(default_settings(Format::V6), &[info("a", &[]), info("c", &[])]).unwrap();
        assert!(out.contains(r#"<node id="ModOrder"><children><node id="Module"><attribute id="UUID" type="FixedString" value="a"/></node>"#));
        assert!(out.contains(r#"value="GustavDev""#));
        assert_eq!(lsx_order(&out).unwrap(), ["a", "c"]);
        for format in [Format::V7, Format::V8] { assert!(merge(default_settings(format), &[info("a", &[])]).unwrap().contains(r#"type="guid" value="a""#)); }
    }
    #[test]
    fn imports_reorder_known_modules_and_keep_the_rest() {
        let paks = vec![("x.pak".to_string(), vec![]), ("a.pak".to_string(), vec![info("A1", &[])]), ("b.pak".to_string(), vec![info("b1", &[])]), ("c.pak".to_string(), vec![info("c1", &[])])];
        let json = r#"{"Order":[{"UUID":"c1","Name":"C"},{"UUID":"a1"}]}"#;
        assert_eq!(reorder(&paks, &bg3mm_order(json).unwrap()), ["c.pak", "a.pak", "x.pak", "b.pak"]);
        assert_eq!(bg3mm_order(r#"[{"UUID":"B1"}]"#).unwrap(), ["b1"]);
        assert!(bg3mm_order(r#"{"Order":[]}"#).is_err() && bg3mm_order("x").is_err());
    }
    #[test]
    fn split_packages_read_metadata_from_their_part_and_hide_the_parts() {
        let dir = std::env::temp_dir().join(format!("peligames-bg3-split-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let meta = br#"<?xml version="1.0" encoding="UTF-8"?><save><region id="Config"><node id="root"><children><node id="Dependencies"><children><node id="ModuleShortDesc"><attribute id="Name" type="LSString" value="Base"/><attribute id="UUID" type="guid" value="11111111-1111-1111-1111-111111111111"/></node><node id="ModuleShortDesc"><attribute id="Name" type="LSString" value="GustavX"/><attribute id="UUID" type="guid" value="cb555efe-2d9e-131f-8195-a89329d218ea"/></node></children></node><node id="ModuleInfo"><attribute id="Folder" type="LSString" value="Split"/><attribute id="Name" type="LSString" value="Split Mod"/><attribute id="UUID" type="guid" value="22222222-2222-2222-2222-222222222222"/><attribute id="Version64" type="int64" value="5"/></node></children></node></region></save>"#;
        let mut entry = vec![0u8; 272];
        entry[..19].copy_from_slice(b"Mods/Split/meta.lsx");
        entry[262] = 1; // part 1, uncompressed, at offset 0
        entry[264..268].copy_from_slice(&(meta.len() as u32).to_le_bytes());
        entry[268..272].copy_from_slice(&(meta.len() as u32).to_le_bytes());
        // LZ4 block made of literals only: token 0xF0, length 15 + 255 + 2.
        let mut list = vec![0xF0, 255, 2];
        list.extend_from_slice(&entry);
        let mut pak = b"LSPK".to_vec();
        pak.extend_from_slice(&18u32.to_le_bytes());
        pak.extend_from_slice(&16u64.to_le_bytes());
        pak.extend_from_slice(&1u32.to_le_bytes());
        pak.extend_from_slice(&(list.len() as u32).to_le_bytes());
        pak.extend_from_slice(&list);
        fs::write(dir.join("Split.pak"), &pak).unwrap();
        assert!(metadata(&dir.join("Split.pak")).err().unwrap().contains("Split_1.pak"));
        fs::write(dir.join("Split_1.pak"), meta).unwrap();
        fs::write(dir.join("Other_2.pak"), b"").unwrap();
        let infos = metadata(&dir.join("Split.pak")).unwrap();
        assert_eq!((infos[0].uuid.as_str(), infos[0].name.as_str()), ("22222222-2222-2222-2222-222222222222", "Split Mod"));
        // The game's own modules are not listed as dependencies.
        assert_eq!(infos[0].dependencies, [("11111111-1111-1111-1111-111111111111".to_string(), "Base".to_string())]);
        let names: Vec<_> = paks_in(&dir).unwrap().into_iter().map(|(name, _)| name).collect();
        assert_eq!(names, ["Other_2.pak", "Split.pak"], "a parte some, um nome parecido sem o primeiro arquivo fica");
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn file_version_is_read_from_the_resource_section() {
        // Minimal PE: DOS header, PE header, one .rsrc section holding VS_FIXEDFILEINFO 4.67.58.0.
        let mut pe = vec![0u8; 0x200];
        pe[..2].copy_from_slice(b"MZ");
        pe[60..64].copy_from_slice(&64u32.to_le_bytes());
        pe[64..68].copy_from_slice(b"PE\0\0");
        pe[64 + 6..64 + 8].copy_from_slice(&1u16.to_le_bytes());
        let section = 64 + 24;
        pe[section..section + 5].copy_from_slice(b".rsrc");
        pe[section + 16..section + 20].copy_from_slice(&0x40u32.to_le_bytes());
        pe[section + 20..section + 24].copy_from_slice(&0x100u32.to_le_bytes());
        pe[0x110..0x114].copy_from_slice(&[0xBD, 0x04, 0xEF, 0xFE]);
        pe[0x118..0x11C].copy_from_slice(&((4u32 << 16) | 67).to_le_bytes());
        pe[0x11C..0x120].copy_from_slice(&(58u32 << 16).to_le_bytes());
        let dir = std::env::temp_dir().join(format!("peligames-bg3-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(dir.join("bin")).unwrap();
        fs::write(dir.join("bin/bg3.exe"), &pe).unwrap();
        assert_eq!(format(&dir), Format::V8);
        pe[0x118..0x11C].copy_from_slice(&((4u32 << 16) | 58).to_le_bytes());
        fs::write(dir.join("bin/bg3.exe"), &pe).unwrap();
        assert_eq!(format(&dir), Format::V7);
        pe[0x11C..0x120].copy_from_slice(&(10u32 << 16).to_le_bytes());
        fs::write(dir.join("bin/bg3.exe"), &pe).unwrap();
        assert_eq!(format(&dir), Format::V6);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(format(&dir), Format::V8, "sem executável legível vale o formato atual");
    }
}
