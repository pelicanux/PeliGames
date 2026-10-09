//! Bounded Larian package metadata reader and modsettings merger (no executable tools).
use super::nexus_local::NexusGame;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
fn lz4(input: &[u8], limit: usize) -> Result<Vec<u8>, String> {
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
        if back == 0 || back > out.len() || n > limit.saturating_sub(out.len()) {
            return Err("Referência LZ4 inválida.".into());
        }
        for _ in 0..n {
            out.push(out[out.len() - back]);
        }
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
fn chunk(file: &mut fs::File, offset: u64, size: usize, limit: usize) -> Result<Vec<u8>, String> {
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
struct Info {
    uuid: String,
    folder: String,
    name: String,
    version: String,
    md5: String,
}
fn metadata(path: &Path) -> Result<Vec<Info>, String> {
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
        if part != 0 {
            return Err("Metadados em pacote LSPK multipartes exigem tratamento adicional.".into());
        }
        let bytes = decompress(
            &chunk(&mut file, start, size, 2 * 1024 * 1024)?,
            flags,
            2 * 1024 * 1024,
        )?;
        let xml = String::from_utf8(bytes).map_err(|_| "Metadados BG3 não são UTF-8.")?;
        let mut tree = parse(xml.trim_start_matches('\u{feff}'))?;
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
        });
    }
    Ok(result)
}
pub(super) fn activation(
    game: &NexusGame,
    runtime: &Path,
    data: &str,
    original: &str,
    old: &BTreeMap<String, String>,
) -> Result<String, String> {
    let game_root = Path::new(game.game["directory"].as_str().ok_or("Pasta ausente.")?);
    let (target, _) =
        super::nexus_targets::root(game_root, &game.compat_data, &format!("{data}/dummy"))?;
    // root() already selects AppData/Local; append the declared game's Mods subfolder.
    let (_, relative) = super::nexus_targets::root(game_root, &game.compat_data, data)?;
    let target = target.join(relative);
    let mut previous = BTreeSet::new();
    for path in old
        .keys()
        .filter(|p| p.starts_with(&format!("{data}/")) && p.to_ascii_lowercase().ends_with(".pak"))
    {
        let name = path.rsplit('/').next().unwrap();
        if target.join(name).is_file() {
            previous.extend(metadata(&target.join(name))?.into_iter().map(|i| i.uuid));
        }
    }
    let mut infos = Vec::new();
    if runtime.join(data).is_dir() {
        for entry in fs::read_dir(runtime.join(data))
            .map_err(|e| e.to_string())?
            .filter_map(Result::ok)
        {
            if entry
                .path()
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("pak"))
            {
                infos.extend(metadata(&entry.path())?);
            }
        }
    }
    let mut unique = BTreeSet::new();
    if infos.iter().any(|i| !unique.insert(i.uuid.clone())) {
        return Err("Dois pacotes BG3 possuem o mesmo UUID de módulo.".into());
    }
    if original.trim().is_empty() {
        return Err("Inicie Baldur’s Gate 3 uma vez nesse prefixo para criar o modsettings.lsx correto para a versão instalada.".into());
    }
    let mut tree = parse(original.trim_start_matches('\u{feff}'))?;
    if tree.find("ModOrder").is_none() {
        tree.find("root")
            .ok_or("Raiz ModuleSettings ausente.")?
            .children()
            .push(node(
                "ModOrder",
                vec![Xml {
                    name: "children".into(),
                    ..Default::default()
                }],
            ));
    }
    for key in ["ModOrder", "Mods"] {
        let children = tree
            .find(key)
            .ok_or("modsettings.lsx existente não contém os nós esperados; preservado.")?
            .children();
        children.retain(|n| {
            !n.value("UUID")
                .is_some_and(|uuid| previous.contains(uuid) || unique.contains(uuid))
        });
        for info in &infos {
            let fields = if key == "ModOrder" {
                vec![attr("UUID", "FixedString", &info.uuid)]
            } else {
                vec![
                    attr("Folder", "LSString", &info.folder),
                    attr("MD5", "LSString", &info.md5),
                    attr("Name", "LSString", &info.name),
                    attr("UUID", "FixedString", &info.uuid),
                    attr("Version64", "int64", &info.version),
                ]
            };
            children.push(node(
                if key == "ModOrder" {
                    "Module"
                } else {
                    "ModuleShortDesc"
                },
                fields,
            ));
        }
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}\n",
        tree.render()
    ))
}
