//! XML FOMOD selection and file planning. No scripts or external XML entities are executed.
use super::nexus_modules::Definition;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::Path,
};
#[derive(Default, Debug)]
struct Node {
    id: usize,
    name: String,
    attrs: BTreeMap<String, String>,
    text: String,
    children: Vec<Node>,
}
impl Node {
    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }
    fn attr(&self, name: &str) -> &str {
        self.attrs.get(name).map(String::as_str).unwrap_or("")
    }
    fn value(&self, name: &str) -> String {
        self.child(name)
            .map(|n| n.text.trim().into())
            .unwrap_or_default()
    }
}
fn xml(bytes: &[u8]) -> Result<Node, String> {
    if bytes.len() > 2 * 1024 * 1024 {
        return Err("O instalador FOMOD excede 2 MB.".into());
    }
    let text = if bytes.starts_with(&[255, 254]) || bytes.starts_with(&[254, 255]) {
        if bytes.len() % 2 != 0 {
            return Err("XML UTF-16 inválido.".into());
        }
        let little = bytes[0] == 255;
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|b| {
                if little {
                    u16::from_le_bytes([b[0], b[1]])
                } else {
                    u16::from_be_bytes([b[0], b[1]])
                }
            })
            .collect::<Vec<_>>();
        String::from_utf16(&units).map_err(|_| "XML UTF-16 inválido.")?
    } else {
        String::from_utf8(bytes.to_vec()).map_err(|_| "O XML deve estar em UTF-8 ou UTF-16.")?
    };
    let mut reader = quick_xml::Reader::from_str(text.trim_start_matches('\u{feff}'));
    let mut stack = vec![Node::default()];
    let mut count = 0usize;
    loop {
        use quick_xml::events::Event;
        let event = reader
            .read_event()
            .map_err(|e| format!("XML FOMOD inválido: {e}"))?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(start) | Event::Empty(start) => {
                count += 1;
                if count > 20000 || stack.len() > 64 {
                    return Err("Estrutura FOMOD grande demais.".into());
                }
                let mut node = Node {
                    id: count,
                    name: String::from_utf8(start.name().as_ref().to_vec())
                        .map_err(|_| "Tag inválida.")?,
                    ..Default::default()
                };
                for attr in start.attributes() {
                    let attr = attr.map_err(|e| e.to_string())?;
                    node.attrs.insert(
                        String::from_utf8(attr.key.as_ref().to_vec())
                            .map_err(|_| "Atributo inválido.")?,
                        attr.decoded_and_normalized_value(
                            quick_xml::XmlVersion::Explicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|e| e.to_string())?
                        .into_owned(),
                    );
                }
                if empty {
                    stack.last_mut().unwrap().children.push(node);
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                if stack.len() <= 1 {
                    return Err("XML FOMOD inválido.".into());
                }
                let node = stack.pop().unwrap();
                stack.last_mut().unwrap().children.push(node);
            }
            Event::Text(value) => stack.last_mut().unwrap().text.push_str(
                &quick_xml::escape::unescape(&value.decode().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?,
            ),
            Event::CData(value) => stack
                .last_mut()
                .unwrap()
                .text
                .push_str(&value.decode().map_err(|e| e.to_string())?),
            Event::GeneralRef(value) => {
                let name = value.decode().map_err(|e| e.to_string())?;
                let escaped = format!("&{name};");
                stack.last_mut().unwrap().text.push_str(
                    &quick_xml::escape::unescape(&escaped)
                        .map_err(|_| "Entidade XML não permitida.")?,
                );
            }
            Event::DocType(_) => {
                return Err("DTD e entidades externas não são permitidas no FOMOD.".into())
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if stack.len() != 1 || stack[0].children.len() != 1 || stack[0].children[0].name != "config" {
        return Err("Estrutura FOMOD inválida.".into());
    }
    let root = stack.remove(0).children.remove(0);
    fn supported(node: &Node) -> Result<(), String> {
        let allowed = [
            "config",
            "moduleName",
            "moduleImage",
            "moduleDependencies",
            "requiredInstallFiles",
            "installSteps",
            "installStep",
            "visible",
            "optionalFileGroups",
            "group",
            "plugins",
            "plugin",
            "description",
            "image",
            "files",
            "file",
            "folder",
            "typeDescriptor",
            "type",
            "dependencyType",
            "defaultType",
            "patterns",
            "pattern",
            "dependencies",
            "flagDependency",
            "fileDependency",
            "gameDependency",
            "fommDependency",
            "foseDependency",
            "scriptExtenderDependency",
            "conditionFlags",
            "flag",
            "conditionalFileInstalls",
        ];
        if !allowed.contains(&node.name.as_str()) {
            return Err(format!("Elemento FOMOD ainda não suportado: {}", node.name));
        }
        for child in &node.children {
            supported(child)?;
        }
        Ok(())
    }
    supported(&root)?;
    Ok(root)
}
#[derive(Serialize)]
pub struct OptionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub selected: bool,
}
#[derive(Serialize)]
pub struct Group {
    pub name: String,
    pub kind: String,
    pub options: Vec<OptionInfo>,
}
#[derive(Serialize)]
pub struct Step {
    pub name: String,
    pub groups: Vec<Group>,
}
#[derive(Serialize)]
pub struct Installer {
    pub name: String,
    pub steps: Vec<Step>,
}
pub(super) struct Resolution {
    pub installer: Option<Installer>,
    pub selection: Vec<String>,
    pub issues: Vec<String>,
    pub files: Option<Vec<(String, String)>>,
}
fn ordered<'a>(node: Option<&'a Node>, tag: &str) -> Result<Vec<&'a Node>, String> {
    let Some(node) = node else {
        return Ok(Vec::new());
    };
    let mut items = node
        .children
        .iter()
        .filter(|n| n.name == tag)
        .collect::<Vec<_>>();
    match node.attr("order") {
        "" | "Explicit" => {}
        "Ascending" => items.sort_by_key(|n| n.attr("name").to_lowercase()),
        "Descending" => items.sort_by_key(|n| std::cmp::Reverse(n.attr("name").to_lowercase())),
        _ => return Err("Ordenação FOMOD desconhecida.".into()),
    }
    Ok(items)
}
fn relative(value: &str, empty: bool) -> Result<String, String> {
    let value = value.replace('\\', "/");
    let value = value.trim_end_matches('/');
    if empty && value.is_empty() {
        return Ok(String::new());
    }
    super::nexus_archive::safe_name(value)
}
/// What conditions can inspect: the game's mod folder and, for Bethesda games, the plugin list.
struct Env<'a> { root: std::path::PathBuf, plugins: Option<&'a super::nexus_loot::PluginOracle<'a>> }
fn condition(node: &Node, flags: &BTreeMap<String, String>, env: &Env) -> Result<bool, String> {
    let root = env.root.as_path();
    match node.name.as_str() {
        "dependencies" | "moduleDependencies" | "visible" => {
            let values = node
                .children
                .iter()
                .map(|n| condition(n, flags, env))
                .collect::<Result<Vec<_>, _>>()?;
            match node.attr("operator") {
                "" | "And" => Ok(values.iter().all(|v| *v)),
                "Or" => Ok(values.iter().any(|v| *v)),
                _ => Err("Operador FOMOD desconhecido.".into()),
            }
        }
        "flagDependency" => Ok(flags
            .get(node.attr("flag"))
            .is_some_and(|v| v == node.attr("value"))),
        "fileDependency" => {
            let file = relative(node.attr("file"), false)?;
            let path = root.join(&file);
            let present =
                path.is_file() && fs::canonicalize(&path).is_ok_and(|p| p.starts_with(root));
            let is_plugin = !file.contains('/') && ["esp", "esm", "esl"]
                .iter()
                .any(|ext| file.to_ascii_lowercase().ends_with(&format!(".{ext}")));
            if is_plugin {
                // Plugins are judged by the load order: Active, Inactive (present but disabled) or Missing.
                use super::nexus_loot::PluginState;
                let Some(plugins) = env.plugins else {
                    if node.attr("state") == "Missing" { return Ok(!present); }
                    return Err("Este FOMOD precisa consultar a ativação de plugins Bethesda, indisponível para este jogo.".into());
                };
                let state = plugins.state(&file)?;
                return match node.attr("state") {
                    "Active" => Ok(state == PluginState::Active),
                    "Inactive" => Ok(state == PluginState::Inactive),
                    "Missing" => Ok(state == PluginState::Missing),
                    _ => Err("Estado de dependência FOMOD desconhecido.".into()),
                };
            }
            match node.attr("state") {
                "Active" => Ok(present),
                "Missing" => Ok(!present),
                "Inactive" => Ok(false),
                _ => Err("Estado de dependência FOMOD desconhecido.".into()),
            }
        }
        _ => Err(format!(
            "Dependência FOMOD ainda não suportada: {}. Nenhum arquivo foi aplicado.",
            node.name
        )),
    }
}
fn plugin_kind(
    plugin: &Node,
    flags: &BTreeMap<String, String>,
    env: &Env,
) -> Result<String, String> {
    let Some(descriptor) = plugin.child("typeDescriptor") else {
        return Ok("Optional".into());
    };
    if let Some(kind) = descriptor.child("type") {
        return Ok(kind.attr("name").into());
    }
    let dependency = descriptor
        .child("dependencyType")
        .ok_or("Tipo FOMOD desconhecido.")?;
    for pattern in ordered(dependency.child("patterns"), "pattern")? {
        if condition(
            pattern
                .child("dependencies")
                .ok_or("Condição FOMOD ausente.")?,
            flags,
            env,
        )? {
            return Ok(pattern
                .child("type")
                .ok_or("Tipo FOMOD ausente.")?
                .attr("name")
                .into());
        }
    }
    Ok(dependency
        .child("defaultType")
        .ok_or("Tipo FOMOD padrão ausente.")?
        .attr("name")
        .into())
}
struct FileRule {
    source: String,
    destination: String,
    folder: bool,
    destination_folder: bool,
    priority: i64,
}
fn rules(
    node: Option<&Node>,
    selected: bool,
    usable: bool,
    out: &mut Vec<FileRule>,
) -> Result<(), String> {
    if let Some(node) = node {
        for file in &node.children {
            if !["file", "folder"].contains(&file.name.as_str()) {
                return Err("Regra de arquivos FOMOD desconhecida.".into());
            }
            let boolean = |attr: &str| -> Result<bool, String> {
                match file.attr(attr) {
                    "" | "false" | "0" => Ok(false),
                    "true" | "1" => Ok(true),
                    _ => Err("Atributo booleano FOMOD inválido.".into()),
                }
            };
            if !selected && !boolean("alwaysInstall")? && !(usable && boolean("installIfUsable")?) {
                continue;
            }
            out.push(FileRule {
                source: relative(file.attr("source"), file.name == "folder")?,
                destination: relative(file.attr("destination"), true)?,
                folder: file.name == "folder",
                destination_folder: file.attr("destination").ends_with(['/', '\\']),
                priority: if file.attr("priority").is_empty() {
                    0
                } else {
                    file.attr("priority")
                        .parse()
                        .map_err(|_| "Prioridade FOMOD inválida.")?
                },
            });
        }
    }
    Ok(())
}
pub(super) fn resolve(
    archive: &mut zip::ZipArchive<fs::File>,
    module: &Definition,
    root: &Path,
    mod_id: &str,
    choices: Option<&[String]>,
    plugins: Option<&super::nexus_loot::PluginOracle>,
) -> Result<Resolution, String> {
    let entries = (0..archive.len())
        .filter_map(|i| match archive.by_index(i) {
            Ok(e) if !e.is_dir() => Some(Ok(e.name().replace('\\', "/"))),
            Ok(_) => None,
            Err(e) => Some(Err(e.to_string())),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let configs = entries
        .iter()
        .filter(|name| {
            name.to_ascii_lowercase()
                .ends_with("fomod/moduleconfig.xml")
        })
        .collect::<Vec<_>>();
    if configs.len() > 1 {
        return Err("O pacote contém mais de um instalador FOMOD.".into());
    }
    let Some(config_path) = configs.first() else {
        if entries.iter().any(|name| {
            name.to_ascii_lowercase().ends_with("fomod/script.cs")
                || name.to_ascii_lowercase().ends_with("fomod/script.xml")
        }) {
            return Err(
                "Este pacote utiliza um instalador FOMOD por script ainda não suportado.".into(),
            );
        }
        if let Some(bain) = super::nexus_bain::resolve(&entries, module, mod_id, choices)? {
            return Ok(bain);
        }
        return Ok(Resolution {
            installer: None,
            selection: Vec::new(),
            issues: Vec::new(),
            files: None,
        });
    };
    let config_index = (0..archive.len())
        .find(|index| {
            archive
                .by_index(*index)
                .is_ok_and(|entry| entry.name().replace('\\', "/") == **config_path)
        })
        .ok_or("XML FOMOD ausente.")?;
    let mut bytes = Vec::new();
    archive
        .by_index(config_index)
        .map_err(|e| e.to_string())?
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let config = xml(&bytes)?;
    let prefix = &config_path[..config_path.len() - "fomod/ModuleConfig.xml".len()];
    let root = fs::canonicalize(root).map_err(|e| e.to_string())?;
    // File conditions are relative to the game's mod folder, as in Vortex: Data
    // (Data Files in Morrowind) for Bethesda games, the game root otherwise.
    let root = match module.activation.as_ref().filter(|rule| matches!(rule.kind, super::nexus_modules::ActivationKind::Bethesda | super::nexus_modules::ActivationKind::Morrowind)) {
        Some(rule) => super::nexus_discovery::locate(&root, &rule.data).and_then(|path| fs::canonicalize(path).ok()).filter(|path| path.starts_with(&root)).unwrap_or_else(|| root.join(&rule.data)),
        None => root,
    };
    let env = Env { root, plugins };
    let requested = choices
        .unwrap_or_default()
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut known = BTreeSet::new();
    let mut flags = BTreeMap::new();
    let mut selection = Vec::new();
    let mut issues = Vec::new();
    let mut steps = Vec::new();
    let mut files = Vec::new();
    if let Some(dependencies) = config.child("moduleDependencies") {
        if !condition(dependencies, &flags, &env)? {
            issues.push(
                "As dependências declaradas pelo instalador FOMOD não estão satisfeitas.".into(),
            );
        }
    }
    rules(config.child("requiredInstallFiles"), true, true, &mut files)?;
    for step in ordered(config.child("installSteps"), "installStep")? {
        let visible = step
            .child("visible")
            .map(|v| condition(v, &flags, &env))
            .transpose()?
            .unwrap_or(true);
        let mut groups = Vec::new();
        for group in ordered(step.child("optionalFileGroups"), "group")? {
            let kind = group.attr("type");
            if ![
                "SelectAll",
                "SelectAny",
                "SelectAtLeastOne",
                "SelectAtMostOne",
                "SelectExactlyOne",
            ]
            .contains(&kind)
            {
                return Err(format!("Grupo FOMOD desconhecido: {kind}"));
            }
            let mut options = Vec::new();
            let plugins = ordered(group.child("plugins"), "plugin")?;
            let preferred = if visible
                && choices.is_none()
                && ["SelectExactlyOne", "SelectAtMostOne"].contains(&kind)
            {
                plugins
                    .iter()
                    .map(|plugin| plugin_kind(plugin, &flags, &env).map(|kind| (plugin.id, kind)))
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .find(|(_, kind)| kind == "Recommended")
                    .map(|(id, _)| id)
            } else {
                None
            };
            for plugin in plugins {
                let id = format!("option-{}", plugin.id);
                known.insert(id.clone());
                if !visible {
                    continue;
                }
                let ptype = plugin_kind(plugin, &flags, &env)?;
                if ![
                    "Required",
                    "Optional",
                    "Recommended",
                    "NotUsable",
                    "CouldBeUsable",
                ]
                .contains(&ptype.as_str())
                {
                    return Err(format!("Tipo de opção FOMOD desconhecido: {ptype}"));
                }
                let usable = ptype != "NotUsable";
                let selected = usable
                    && (kind == "SelectAll"
                        || ptype == "Required"
                        || requested.contains(&id)
                        || choices.is_none()
                            && ptype == "Recommended"
                            && (preferred.is_none() || preferred == Some(plugin.id)));
                if requested.contains(&id) && !usable {
                    issues.push(format!("Opção indisponível: {}", plugin.attr("name")));
                }
                if selected {
                    selection.push(id.clone());
                    if let Some(values) = plugin.child("conditionFlags") {
                        for value in &values.children {
                            if value.name != "flag" || value.attr("name").is_empty() {
                                return Err("Flag FOMOD inválida.".into());
                            }
                            flags.insert(value.attr("name").into(), value.text.trim().into());
                        }
                    }
                }
                rules(plugin.child("files"), selected, usable, &mut files)?;
                options.push(OptionInfo {
                    id,
                    name: plugin.attr("name").into(),
                    description: plugin.value("description"),
                    kind: ptype,
                    selected,
                });
            }
            if !visible {
                continue;
            }
            let selected = options.iter().filter(|o| o.selected).count();
            if kind == "SelectExactlyOne" && selected != 1
                || kind == "SelectAtLeastOne" && selected == 0
                || kind == "SelectAtMostOne" && selected > 1
                || kind == "SelectAll" && options.iter().any(|o| !o.selected)
            {
                let instruction = match kind {
                    "SelectExactlyOne" => "Escolha exatamente uma opção",
                    "SelectAtLeastOne" => "Escolha pelo menos uma opção",
                    "SelectAtMostOne" => "Escolha no máximo uma opção",
                    _ => "Confira as opções obrigatórias",
                };
                issues.push(format!("{instruction}: {}.", group.attr("name")));
            }
            groups.push(Group {
                name: group.attr("name").into(),
                kind: kind.into(),
                options,
            });
        }
        if visible {
            steps.push(Step {
                name: step.attr("name").into(),
                groups,
            });
        }
    }
    if requested.iter().any(|id| !known.contains(id)) {
        return Err("Seleção FOMOD inválida ou pertencente a outro pacote.".into());
    }
    if let Some(conditional) = config.child("conditionalFileInstalls") {
        for pattern in ordered(conditional.child("patterns"), "pattern")? {
            if condition(
                pattern
                    .child("dependencies")
                    .ok_or("Condição FOMOD ausente.")?,
                &flags,
                &env,
            )? {
                rules(pattern.child("files"), true, true, &mut files)?;
            }
        }
    }
    let mut output = BTreeMap::<String, (i64, String, String)>::new();
    for rule in files {
        let source = format!("{prefix}{}", rule.source)
            .trim_end_matches('/')
            .to_string();
        let source_lower = source.to_ascii_lowercase();
        let mut found = false;
        for name in &entries {
            let lower = name.to_ascii_lowercase();
            let suffix = if rule.folder {
                if source.is_empty() {
                    Some(name.as_str())
                } else {
                    lower
                        .strip_prefix(&format!("{source_lower}/"))
                        .map(|_| &name[source.len() + 1..])
                }
            } else {
                (lower == source_lower).then_some("")
            };
            let Some(suffix) = suffix else {
                continue;
            };
            found = true;
            let destination = if rule.folder {
                if rule.destination.is_empty() {
                    suffix.into()
                } else {
                    format!("{}/{suffix}", rule.destination)
                }
            } else {
                if rule.destination.is_empty() {
                    name.rsplit('/').next().unwrap().into()
                } else if rule.destination_folder {
                    format!("{}/{}", rule.destination, name.rsplit('/').next().unwrap())
                } else {
                    rule.destination.clone()
                }
            };
            let target = if module.allowed(&destination) {
                destination
            } else {
                module
                    .route_package(&destination, mod_id, &entries)?
                    .ok_or_else(|| {
                        format!("Destino FOMOD não reconhecido pelo módulo do jogo: {destination}")
                    })?
            };
            if !module.allowed(&target) {
                return Err(format!("Destino FOMOD fora das regras do jogo: {target}"));
            }
            let key = target.to_ascii_lowercase();
            if let Some((priority, previous, _)) = output.get(&key) {
                if *priority > rule.priority {
                    continue;
                }
                if *priority == rule.priority && previous != name {
                    return Err(format!("Alternativas FOMOD conflitantes: {target}"));
                }
            }
            output.insert(key, (rule.priority, name.clone(), target));
        }
        if !found {
            return Err(format!(
                "Arquivo declarado pelo FOMOD não encontrado: {source}"
            ));
        }
    }
    Ok(Resolution {
        installer: Some(Installer {
            name: config.value("moduleName"),
            steps,
        }),
        selection,
        issues,
        files: Some(
            output
                .into_values()
                .map(|(_, source, target)| (source, target))
                .collect(),
        ),
    })
}

/// Redeploy the confirmed instructions without re-evaluating conditions against a changed installation.
// Recheck destination boundaries and archive members without rerunning installer selection.
pub(super) fn validate_installation_files(
    archive: &mut zip::ZipArchive<fs::File>,
    module: &Definition,
    files: &[(String, String)],
) -> Result<(), String> {
    if files.is_empty() || files.len() > 10000 {
        return Err("Plano de instalação inválido.".into());
    }
    let sources = (0..archive.len())
        .filter_map(|index| match archive.by_index(index) {
            Ok(entry) if !entry.is_dir() => Some(Ok(entry.name().replace('\\', "/"))),
            Ok(_) => None,
            Err(error) => Some(Err(error.to_string())),
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let mut destinations = BTreeSet::new();
    for (source, target) in files {
        if !sources.contains(source)
            || !module.allowed(target)
            || !destinations.insert(target.to_ascii_lowercase())
        {
            return Err(format!("Plano de instalação inválido: {source} → {target}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod data_folder_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn bethesda_file_conditions_are_relative_to_data() {
        let base = std::env::temp_dir().join(format!("peligames-fomod-data-{}", uuid::Uuid::new_v4()));
        let root = base.join("game");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("SkyrimSE.exe"), b"MZ").unwrap();
        let path = base.join("mod.zip");
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        writer.start_file("fomod/ModuleConfig.xml", zip::write::FileOptions::default()).unwrap();
        writer.write_all(br#"<config><moduleName>Patch</moduleName><conditionalFileInstalls><patterns><pattern><dependencies operator="And"><fileDependency file="SKSE\Plugins\x.dll" state="Active"/></dependencies><files><file source="patch.esp" destination="patch.esp"/></files></pattern></patterns></conditionalFileInstalls></config>"#).unwrap();
        writer.start_file("patch.esp", zip::write::FileOptions::default()).unwrap();
        writer.write_all(b"TES4").unwrap();
        writer.finish().unwrap();
        let module = super::super::nexus_modules::registry().definitions.into_iter().find(|m| m.name == "Skyrim Special Edition").unwrap();
        let id = "00000000-0000-4000-8000-000000000001";
        let files = |archive: &mut zip::ZipArchive<fs::File>| resolve(archive, &module, &root, id, None, None).unwrap().files.unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        assert!(files(&mut archive).is_empty(), "sem SKSE o patch não entra");
        fs::create_dir_all(root.join("Data/SKSE/Plugins")).unwrap();
        fs::write(root.join("Data/SKSE/Plugins/x.dll"), b"MZ").unwrap();
        assert_eq!(files(&mut archive), [("patch.esp".to_string(), "Data/patch.esp".to_string())]);
        fs::remove_dir_all(base).unwrap();
    }
}

#[cfg(test)]
mod installation_plan_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn approved_mapping_survives_route_changes_but_not_unsafe_destinations() {
        let path = std::env::temp_dir().join(format!("peligames-plan-{}.zip", uuid::Uuid::new_v4()));
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        writer.start_file("wrapper/example.archive", zip::write::FileOptions::default()).unwrap();
        writer.write_all(b"fixture").unwrap();
        writer.finish().unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(&path).unwrap()).unwrap();
        let mut module = super::super::nexus_modules::registry().definitions.into_iter()
            .find(|module| module.domain == "cyberpunk2077").unwrap();
        module.routes.clear();
        let source = "wrapper/example.archive".to_string();
        let target = "archive/pc/mod/example.archive".to_string();
        assert!(validate_installation_files(&mut archive, &module, &[(source.clone(), target.clone())]).is_ok());
        for invalid in ["../outside.archive", "/tmp/outside.archive", "bin/x64/Cyberpunk2077.exe", "unapproved/config.ini"] {
            assert!(validate_installation_files(&mut archive, &module, &[(source.clone(), invalid.into())]).is_err());
        }
        assert!(validate_installation_files(&mut archive, &module, &[("missing.archive".into(), target.clone())]).is_err());
        assert!(validate_installation_files(&mut archive, &module, &[(source.clone(), target.clone()), (source, target)]).is_err());
        drop(archive);
        fs::remove_file(path).unwrap();
    }
}
