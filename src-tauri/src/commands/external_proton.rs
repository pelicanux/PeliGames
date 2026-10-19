//! Narrow per-game Proton adapters. Discovery and executable/prefix settings stay read-only.
use super::ExternalGame;
use saphyr::{LoadableYamlNode, ScalarOwned, Yaml, YamlEmitter, YamlOwned};
use serde_json::{json, Map, Value};
use std::{fs, path::Path};

fn read(path: &Path) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|_| format!("Configuração não encontrada: {}",path.display()))?;
    if !metadata.is_file() || metadata.len() > 4 * 1024 * 1024 { return Err("Configuração externa inválida ou grande demais.".into()); }
    fs::read_to_string(path).map_err(|_| "Não foi possível ler a configuração do launcher.".into())
}

pub(super) fn save(path: &Path, before: &str, after: &str) -> Result<(), String> {
    if before == after { return Ok(()); }
    let path = path.canonicalize().map_err(|e|e.to_string())?;
    let parent = path.parent().ok_or("Configuração sem diretório.")?;
    let backups = parent.join(".peligames-backups");
    if backups.is_symlink() { return Err("Diretório de backup não pode ser um link simbólico.".into()); }
    fs::create_dir_all(&backups).map_err(|e|e.to_string())?;
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&backups, fs::Permissions::from_mode(0o700)).map_err(|e|e.to_string())?;
    }
    let backup = backups.join(format!("{}-{}.bak",path.file_name().unwrap().to_string_lossy(),uuid::Uuid::new_v4()));
    let temporary = parent.join(format!(".peligames-{}.part",uuid::Uuid::new_v4()));
    let result = (|| {
        use std::io::Write;
        let mut backup_file = fs::OpenOptions::new().write(true).create_new(true).open(&backup).map_err(|e|e.to_string())?;
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; backup_file.set_permissions(fs::Permissions::from_mode(0o600)).map_err(|e|e.to_string())?; }
        backup_file.write_all(before.as_bytes()).map_err(|e|e.to_string())?;
        backup_file.sync_all().map_err(|e|e.to_string())?;
        let mut file = fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|e|e.to_string())?;
        file.set_permissions(fs::metadata(&path).map_err(|e|e.to_string())?.permissions()).map_err(|e|e.to_string())?;
        file.write_all(after.as_bytes()).map_err(|e|e.to_string())?;
        file.sync_all().map_err(|e|e.to_string())?;
        if read(&path)? != before { return Err("O launcher alterou a configuração durante a operação. Feche-o e tente novamente.".into()); }
        fs::rename(&temporary,&path).map_err(|e|e.to_string())
    })();
    if result.is_err() { let _ = fs::remove_file(temporary); }
    result
}

fn safe_id(value: &str) -> Result<&str, String> {
    if value.is_empty() || value.len() > 200 || value.contains(['/', '\\', '\0']) || value == "." || value == ".." {
        return Err("Identificação do jogo inválida.".into());
    }
    Ok(value)
}

fn yaml_set(document: &mut YamlOwned, path: &[&str], value: YamlOwned) -> Result<(), String> {
    let map = document.as_mapping_mut().ok_or("Configuração YAML deve ser um mapa.")?;
    let key = YamlOwned::Value(ScalarOwned::String(path[0].into()));
    if path.len() == 1 { map.insert(key,value); return Ok(()); }
    if !map.contains_key(&key) { map.insert(key.clone(),YamlOwned::Mapping(Default::default())); }
    yaml_set(map.get_mut(&key).unwrap(),&path[1..],value)
}
fn yaml_string(value: &str) -> YamlOwned { YamlOwned::Value(ScalarOwned::String(value.into())) }
fn yaml_update(path: &Path, changes: &[(&[&str], String)]) -> Result<(), String> {
    let before = read(path)?;
    let mut documents = YamlOwned::load_from_str(&before).map_err(|_| "Configuração YAML inválida.")?;
    if documents.len() != 1 { return Err("A configuração deve conter um único documento YAML.".into()); }
    for (keys,value) in changes { yaml_set(&mut documents[0],keys,yaml_string(value))?; }
    let mut after = String::new();
    YamlEmitter::new(&mut after).dump(&Yaml::from(&documents[0])).map_err(|e|e.to_string())?;
    after.push('\n');
    save(path,&before,&after)
}

// Valve KeyValues text subset, including comments and escaped quotes. Reject
// unsupported/ambiguous input instead of rewriting a partially parsed config.
pub(super) fn vdf(text: &str) -> Result<Value,String> {
    let bytes = text.as_bytes(); let mut i = 0; let mut tokens = Vec::new();
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() { i += 1; continue; }
        if bytes[i..].starts_with(b"//") { while i < bytes.len() && bytes[i] != b'\n' { i+=1; } continue; }
        if matches!(bytes[i],b'{'|b'}') { tokens.push((bytes[i] as char).to_string()); i+=1; continue; }
        if bytes[i] != b'"' { return Err("Formato VDF não suportado; a configuração foi preservada.".into()); }
        i+=1; let mut value=Vec::new(); let mut closed=false;
        while i < bytes.len() {
            if bytes[i] == b'"' { i+=1; closed=true; break; }
            if bytes[i] == b'\\' && i+1 < bytes.len() && matches!(bytes[i+1],b'"'|b'\\') { i+=1; }
            value.push(bytes[i]); i+=1;
        }
        if !closed { return Err("Texto VDF incompleto.".into()); }
        tokens.push(format!("s{}",String::from_utf8(value).map_err(|_|"Texto VDF inválido.")?));
    }
    fn map(tokens:&[String],i:&mut usize,depth:usize,nested:bool)->Result<Value,String> {
        if depth>64 { return Err("VDF profundo demais.".into()); }
        let mut result=Map::new();
        while *i < tokens.len() {
            if tokens[*i]=="}" { if !nested { return Err("VDF inválido.".into()); } *i+=1; return Ok(Value::Object(result)); }
            let key=tokens[*i].strip_prefix('s').ok_or("Chave VDF inválida.")?.to_string(); *i+=1;
            let token=tokens.get(*i).ok_or("Valor VDF ausente.")?; *i+=1;
            let value=if token=="{" {map(tokens,i,depth+1,true)?} else {Value::String(token.strip_prefix('s').ok_or("Valor VDF inválido.")?.into())};
            if result.keys().any(|k|k.eq_ignore_ascii_case(&key)) { return Err("Chave VDF duplicada.".into()); }
            result.insert(key,value);
        }
        if nested { return Err("Bloco VDF incompleto.".into()); }
        Ok(Value::Object(result))
    }
    map(&tokens,&mut 0,0,false)
}
fn section<'a>(value:&'a mut Value,keys:&[&str])->Result<&'a mut Value,String> {
    if keys.is_empty() { return Ok(value); }
    let map=value.as_object_mut().ok_or("Seção VDF inválida.")?;
    let key=map.keys().find(|k|k.eq_ignore_ascii_case(keys[0])).cloned().unwrap_or_else(||keys[0].into());
    section(map.entry(key).or_insert_with(||json!({})),&keys[1..])
}
fn vdf_dump(value:&Value)->Result<String,String> {
    fn emit(value:&Value,depth:usize,out:&mut String)->Result<(),String> {
        for (key,value) in value.as_object().ok_or("VDF inválido.")? {
            let escape=|s:&str|s.replace('\\',"\\\\").replace('"',"\\\"");
            out.push_str(&format!("{}\"{}\"", "\t".repeat(depth),escape(key)));
            if let Some(s)=value.as_str() {out.push_str(&format!("\t\"{}\"\n",escape(s)));}
            else {out.push_str(&format!("\n{}{{\n","\t".repeat(depth)));emit(value,depth+1,out)?;out.push_str(&format!("{}}}\n","\t".repeat(depth)));}
        } Ok(())
    }
    let mut out=String::new();emit(value,0,&mut out)?;Ok(out)
}

pub(super) fn steam_config(game: &ExternalGame) -> Result<std::path::PathBuf, String> {
    let home = dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    let roots = super::super::steam_installation::roots(&home, game.flatpak);
    roots.into_iter().map(|p| p.join("config/config.vdf")).find(|p| p.is_file()).ok_or("Configuração Steam indisponível.".into())
}
fn steam(home:&Path,game:&ExternalGame,proton:&Path)->Result<(),String> {
    let id=game.app_id.as_deref().ok_or("Steam AppID ausente.")?;
    if id.parse::<u32>().ok().filter(|n|*n>0).is_none() {return Err("Steam AppID inválido.".into());}
    let roots=super::super::steam_installation::roots(home, game.flatpak);
    let root=roots.into_iter().find(|p|p.join("config/config.vdf").is_file()).ok_or("Configuração Steam indisponível.")?;
    let path=root.join("config/config.vdf"); let before=read(&path)?; let mut data=vdf(&before)?;
    let directory=root.join("compatibilitytools.d");fs::create_dir_all(&directory).map_err(|e|e.to_string())?;
    let name=register_steam_tool(&directory,proton)?;
    let mapping=section(&mut data,&["InstallConfigStore","Software","Valve","Steam","CompatToolMapping"])?;
    let game_mapping=section(mapping,&[id])?;
    *section(game_mapping,&["name"])?=Value::String(name);
    *section(game_mapping,&["config"])?=Value::String(String::new());
    *section(game_mapping,&["priority"])?=Value::String("250".into());
    save(&path,&before,&vdf_dump(&data)?)
}

fn register_steam_tool(directory:&Path,proton:&Path)->Result<String,String> {
    use sha2::{Digest,Sha256};
    // Give the adapter a unique identity, including Valve Proton installations
    // which have no custom compatibilitytool.vdf. Never edit the original runner.
    let digest=format!("{:x}",Sha256::digest(proton.to_string_lossy().as_bytes()));
    let name=format!("peligames-{}",&digest[..16]);
    let target=directory.join(&name);let marker=target.join(".peligames-proton-path");
    if fs::symlink_metadata(&target).is_ok() {
        if target.is_symlink() || fs::read_to_string(&marker).ok().as_deref()!=proton.to_str() {return Err("Diretório de integração Steam ocupado por outra ferramenta.".into());}
    } else {
        fs::create_dir(&target).map_err(|e|e.to_string())?;
        fs::write(&marker,proton.to_string_lossy().as_bytes()).map_err(|e|e.to_string())?;
    }
    for entry in fs::read_dir(proton).map_err(|e|e.to_string())? {
        let entry=entry.map_err(|e|e.to_string())?;let leaf=entry.file_name();
        if leaf=="compatibilitytool.vdf" || leaf==".peligames-proton-path" {continue;}
        let link=target.join(leaf);
        if fs::symlink_metadata(&link).is_ok() {
            if fs::read_link(&link).ok().as_deref()!=Some(entry.path().as_path()) {return Err("Arquivo conflitante na integração Steam.".into());}
        } else {
            #[cfg(unix)] std::os::unix::fs::symlink(entry.path(),&link).map_err(|e|e.to_string())?;
        }
    }
    let display=format!("{} (PeliGames)",proton.file_name().unwrap().to_string_lossy());
    let manifest=json!({"compatibilitytools":{"compat_tools":{name.clone():{"install_path":".","display_name":display,"from_oslist":"windows","to_oslist":"linux"}}}});
    let file=target.join("compatibilitytool.vdf");
    if file.is_symlink() {return Err("Manifesto da integração Steam não pode ser um link.".into());}
    fs::write(file,vdf_dump(&manifest)?).map_err(|e|e.to_string())?;
    Ok(name)
}

pub(super) fn apply(game:&ExternalGame,proton:&Path)->Result<(),String> {
    let home=dirs::home_dir().ok_or("Pasta pessoal indisponível.")?;
    apply_in(&home,game,proton)
}
fn apply_in(home:&Path,game:&ExternalGame,proton:&Path)->Result<(),String> {
    let id=safe_id(game.app_id.as_deref().ok_or("Identificação do jogo ausente.")?)?;
    match game.launcher.as_str() {
        "Steam"=>steam(home,game,proton),
        "Heroic"=>{
            let base=if game.flatpak {home.join(".var/app/com.heroicgameslauncher.hgl/config/heroic")} else {home.join(".config/heroic")};
            let path=base.join("GamesConfig").join(format!("{id}.json")); let before=read(&path)?;
            let mut config:Value=serde_json::from_str(&before).map_err(|_|"Configuração Heroic inválida.")?;
            let settings=config.get_mut(id).and_then(Value::as_object_mut).ok_or("Configuração deste jogo Heroic indisponível.")?;
            settings.insert("wineVersion".into(),json!({"bin":proton.join("proton"),"name":proton.file_name().unwrap().to_string_lossy(),"type":"proton"}));
            // UMU is the supported backend for the selected standalone Proton.
            settings.insert("disableUMU".into(),Value::Bool(false));
            save(&path,&before,&serde_json::to_string_pretty(&config).map_err(|e|e.to_string())?)
        },
        "Faugus"=>{
            let path=if game.flatpak {home.join(".var/app/io.github.Faugus.faugus-launcher/data/faugus-launcher/games.json")} else {home.join(".local/share/faugus-launcher/games.json")};
            let before=read(&path)?; let mut games:Value=serde_json::from_str(&before).map_err(|_|"Configuração Faugus inválida.")?;
            let game=games.as_array_mut().ok_or("Catálogo Faugus inválido.")?.iter_mut().find(|g|g["gameid"]==id).ok_or("Jogo Faugus não encontrado.")?;
            game.as_object_mut().ok_or("Jogo Faugus inválido.")?.insert("runner".into(),Value::String(proton.to_string_lossy().into_owned()));
            save(&path,&before,&serde_json::to_string_pretty(&games).map_err(|e|e.to_string())?)
        },
        "Lutris"=>{
            let data=if game.flatpak {home.join(".var/app/net.lutris.Lutris/data/lutris")} else {home.join(".local/share/lutris")};
            let database=super::lutris_ids(home,game.flatpak);
            let configs:Vec<_>=database.iter().filter(|(_, (game_id,_))|game_id==id).map(|(file,_)|file).collect();
            if configs.len()!=1 {return Err("Não foi possível identificar a configuração única do jogo Lutris.".into());}
            let stem=safe_id(configs[0])?;
            let config_dir=if game.flatpak {home.join(".var/app/net.lutris.Lutris/config/lutris/games")} else {home.join(".config/lutris/games")};
            let candidates=[data.join("games").join(format!("{stem}.yml")),config_dir.join(format!("{stem}.yml"))];
            let existing:Vec<_>=candidates.iter().filter(|p|p.is_file()).collect();
            if existing.len()!=1 {return Err("Configuração Lutris ausente ou ambígua.".into());}
            let umu=crate::core::game_installation::find_umu()?;
            yaml_update(existing[0],&[(&["wine","version"],"custom".into()),(&["wine","custom_wine_path"],umu.to_string_lossy().into_owned()),(&["system","env","PROTONPATH"],proton.to_string_lossy().into_owned())])
        },
        "Bottles"=>{
            // A bottle shares its runner and prefix across all its programs.
            // Do not silently change unrelated programs through a per-game control.
            Err("O Bottles compartilha o runner entre os programas da mesma bottle. Altere essa versão no Bottles para não afetar outros jogos por uma configuração individual.".into())
        },
        _=>Err("Launcher não suporta seleção de Proton por esta integração.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vdf_preserves_other_games_and_global_mapping() {
        let original=r#"// comment
        "InstallConfigStore" {"Software" {"Valve" {"Steam" {"CompatToolMapping" {"0" {"name" "global"} "42" {"name" "other"}} "unrelated" "keep"}}}}"#;
        let mut data=vdf(original).unwrap();
        *section(&mut data,&["installconfigstore","software","valve","steam","CompatToolMapping","22380"] ).unwrap()=json!({"name":"selected","priority":"250"});
        let output=vdf_dump(&data).unwrap();let mut parsed=vdf(&output).unwrap();
        let mapping=section(&mut parsed,&["InstallConfigStore","Software","Valve","Steam","CompatToolMapping"]).unwrap();
        assert_eq!(mapping["0"]["name"],"global");assert_eq!(mapping["42"]["name"],"other");assert_eq!(mapping["22380"]["name"],"selected");
        assert!(vdf("\"a\" { \"b\" \"c\"").is_err());assert!(vdf("\"a\" \"b\" \"a\" \"c\"").is_err());
    }
    #[test]
    fn yaml_changes_preserve_executable_and_prefix() {
        let mut data=YamlOwned::load_from_str("game:\n  exe: game.exe\n  prefix: /games/prefix\nwine:\n  version: old\nsystem:\n  env:\n    KEEP: value\n").unwrap().remove(0);
        yaml_set(&mut data,&["wine","version"],yaml_string("custom")).unwrap();
        yaml_set(&mut data,&["system","env","PROTONPATH"],yaml_string("/selected")).unwrap();
        assert_eq!(data.as_mapping_get("game").unwrap().as_mapping_get("prefix").unwrap().as_str(),Some("/games/prefix"));
        assert_eq!(data.as_mapping_get("system").unwrap().as_mapping_get("env").unwrap().as_mapping_get("KEEP").unwrap().as_str(),Some("value"));
    }
    #[test]
    fn rejects_ids_and_concurrent_file_changes() {
        assert!(safe_id("../escape").is_err());assert!(safe_id("x/y").is_err());
        let dir=std::env::temp_dir().join(format!("peligames-adapter-{}",uuid::Uuid::new_v4()));fs::create_dir(&dir).unwrap();
        let file=dir.join("config.json");fs::write(&file,"current").unwrap();assert!(save(&file,"stale","changed").is_err());assert_eq!(read(&file).unwrap(),"current");fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn steam_registers_selected_runner_without_changing_its_files() {
        let home=std::env::temp_dir().join(format!("peligames-steam-proton-{}",uuid::Uuid::new_v4()));
        let root=home.join(".local/share/Steam");fs::create_dir_all(root.join("config")).unwrap();
        let original="\"InstallConfigStore\" {\"Software\" {\"Valve\" {\"Steam\" {\"CompatToolMapping\" {\"0\" {\"name\" \"global\"} \"42\" {\"name\" \"other\"}}}}}}";
        fs::write(root.join("config/config.vdf"),original).unwrap();
        let runner=home.join("selected-proton");fs::create_dir(&runner).unwrap();fs::write(runner.join("proton"),"original script").unwrap();
        let game=super::super::tests::game("Steam","22380");
        apply_in(&home,&game,&runner).unwrap();
        let mut config=vdf(&read(&root.join("config/config.vdf")).unwrap()).unwrap();
        let mapping=section(&mut config,&["InstallConfigStore","Software","Valve","Steam","CompatToolMapping"]).unwrap();
        let tool=mapping["22380"]["name"].as_str().unwrap();assert!(tool.starts_with("peligames-"));
        assert_eq!(mapping["0"]["name"],"global");assert_eq!(mapping["42"]["name"],"other");
        assert_eq!(fs::read_link(root.join("compatibilitytools.d").join(tool).join("proton")).unwrap(),runner.join("proton"));
        assert_eq!(read(&runner.join("proton")).unwrap(),"original script");assert!(!runner.join("compatibilitytool.vdf").exists());
        assert_eq!(fs::read_dir(root.join("config/.peligames-backups")).unwrap().count(),1);
        fs::remove_dir_all(home).unwrap();
    }
    #[test]
    fn heroic_and_faugus_only_change_selected_game_runner() {
        let home=std::env::temp_dir().join(format!("peligames-json-proton-{}",uuid::Uuid::new_v4()));
        let folder=home.join(".config/heroic/GamesConfig");fs::create_dir_all(&folder).unwrap();
        let path=folder.join("epic-id.json");fs::write(&path,r#"{"epic-id":{"winePrefix":"/original","wineVersion":{"bin":"old","type":"proton"},"targetExe":"game.exe","autoSyncSaves":true},"other":{"keep":true}}"#).unwrap();
        let runner=home.join("chosen");fs::create_dir(&runner).unwrap();
        apply_in(&home,&super::super::tests::game("Heroic","epic-id"),&runner).unwrap();
        let result:Value=serde_json::from_str(&read(&path).unwrap()).unwrap();
        assert_eq!(result["epic-id"]["winePrefix"],"/original");assert_eq!(result["epic-id"]["targetExe"],"game.exe");assert_eq!(result["other"]["keep"],true);
        assert_eq!(result["epic-id"]["wineVersion"]["bin"],runner.join("proton").to_string_lossy().as_ref());
        let folder=home.join(".local/share/faugus-launcher");fs::create_dir_all(&folder).unwrap();let path=folder.join("games.json");
        fs::write(&path,r#"[{"gameid":"one","runner":"old","prefix":"keep","path":"game.exe"},{"gameid":"two","runner":"other"}]"#).unwrap();
        apply_in(&home,&super::super::tests::game("Faugus","one"),&runner).unwrap();
        let result:Value=serde_json::from_str(&read(&path).unwrap()).unwrap();assert_eq!(result[0]["prefix"],"keep");assert_eq!(result[0]["path"],"game.exe");assert_eq!(result[1]["runner"],"other");
        fs::remove_dir_all(home).unwrap();
    }
}
