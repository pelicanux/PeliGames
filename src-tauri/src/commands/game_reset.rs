//! Explicit, per-tab resets for registered installations. Never guesses foreign mod files.
use crate::core::{
    advanced_settings::AdvancedSettings,
    game_reset::{self, Section},
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[derive(Deserialize)]
pub struct Request {
    pub path: String,
    pub external_id: Option<String>,
    pub nexus_id: Option<String>,
    pub section: Section,
    pub gpu_arch: String,
    pub bitness: String,
}
#[derive(Serialize)]
pub struct ResultInfo {
    pub game: serde_json::Value,
    pub completed: Vec<String>,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
enum Target {
    Own(crate::core::installed_library::InstalledEntry),
    External(super::external_library::ExternalGame),
    Nexus(super::nexus_local::NexusGame),
}
impl Target {
    fn directory(&self) -> String {
        match self {
            Self::Own(g) => g.directory.clone(),
            Self::External(g) => g.directory.clone(),
            Self::Nexus(g) => g.game["directory"].as_str().unwrap_or_default().into(),
        }
    }
    fn prefix(&self) -> Option<String> {
        match self {
            Self::Own(g) => Some(g.prefix.clone()),
            Self::External(g) => g.prefix.clone(),
            Self::Nexus(g) => Some(g.compat_data.clone()),
        }
    }
    fn advanced(&self) -> AdvancedSettings {
        match self {
            Self::Own(g) => g.advanced.clone(),
            Self::External(g) => g.advanced.clone(),
            Self::Nexus(g) => g.advanced.clone(),
        }
    }
    fn executable(&self) -> Option<String> {
        match self {
            Self::Own(g) => Some(g.executable.clone()),
            Self::External(g) => g.executable.clone(),
            Self::Nexus(g) => g.game["executable"].as_str().map(str::to_string),
        }
    }
    fn key(&self) -> String {
        match self {
            Self::Own(g) => g.path.clone(),
            Self::External(g) => g.external_id.clone(),
            Self::Nexus(g) => g.game["path"].as_str().unwrap_or_default().into(),
        }
    }
    fn json(&self) -> Result<serde_json::Value, String> {
        match self {
            Self::Own(g) => serde_json::to_value(g),
            Self::External(g) => serde_json::to_value(g),
            Self::Nexus(g) => {
                let mut v = g.game.clone();
                v["advanced"] = serde_json::to_value(&g.advanced).map_err(|e| e.to_string())?;
                return Ok(v);
            }
        }
        .map_err(|e| e.to_string())
    }
}
fn resolve(request: &Request) -> Result<Target, String> {
    if let Some(id) = &request.external_id {
        let g = super::external_library::all()?
            .into_iter()
            .find(|g| &g.external_id == id && g.path == request.path)
            .ok_or("Jogo não encontrado no launcher de origem.")?;
        return Ok(Target::External(g));
    }
    if let Some(g) = crate::core::installed_library::list(false)?
        .into_iter()
        .find(|g| g.path == request.path)
    {
        return Ok(Target::Own(g));
    }
    if let Some(id) = &request.nexus_id {
        let g = super::nexus_local::diagnostic_game(id)?;
        if g.game["path"].as_str() == Some(request.path.as_str()) {
            return Ok(Target::Nexus(g));
        }
    }
    Err("Selecione uma instalação registrada para redefinir.".into())
}
async fn persist(target: Target, settings: AdvancedSettings) -> Result<Target, String> {
    match target {
        Target::Own(g) => tauri::async_runtime::spawn_blocking(move || {
            let next = crate::core::installed_library::update_settings(
                crate::core::installed_library::EntrySettings {
                    path: g.path,
                    name: g.name,
                    proton: g.proton,
                    executable: g.executable,
                    prefix: Some(g.prefix),
                    advanced: Some(settings),
                },
            )?;
            super::launch_profiles::refresh(Path::new(&next.directory), &next.name, &next.advanced)?;
            Ok(Target::Own(next))
        })
        .await
        .map_err(|e| e.to_string())?,
        Target::External(g) => Ok(Target::External(
            super::external_library::configure_external_game(
                super::external_library::ExternalSettings {
                    external_id: g.external_id,
                    advanced: settings,
                    proton: None,
                    directory: None,
                },
            )
            .await?,
        )),
        Target::Nexus(mut g) => {
            super::nexus_launch_options::configure_nexus_advanced(g.id.clone(), settings.clone())
                .await?;
            g.advanced = settings;
            Ok(Target::Nexus(g))
        }
    }
}
#[tauri::command]
pub async fn reset_game_details(
    app: tauri::AppHandle,
    request: Request,
) -> Result<ResultInfo, String> {
    let _busy = super::game_installation::acquire_installer_busy()?;
    if crate::core::game_execution::active() || crate::core::wine_tools::busy() {
        return Err("Feche o jogo e as ferramentas Wine antes de redefinir.".into());
    }
    let mut target = resolve(&request)?;
    let root = PathBuf::from(target.directory())
        .canonicalize()
        .map_err(|_| "A pasta deste jogo não está disponível.")?;
    crate::core::installed_library::ensure_path_writable(&root)?;
    if let Ok(entries) = std::fs::read_dir("/proc") {
        if entries
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .bytes()
                    .all(|b| b.is_ascii_digit())
            })
            .any(|e| {
                std::fs::read_link(e.path().join("exe")).is_ok_and(|exe| exe.starts_with(&root))
            })
        {
            return Err("O jogo ainda está aberto. Encerre-o antes de redefinir.".into());
        }
    }

    if let Some(prefix) = target.prefix().filter(|p| !p.is_empty()) {
        if crate::core::game_execution::prefix_in_use(Path::new(&prefix)) {
            return Err(
                "Este prefixo ainda está em uso. Feche o jogo e as ferramentas Wine.".into(),
            );
        }
    }
    let nexus = match request.nexus_id.as_ref() {
        Some(id) => {
            let g = super::nexus_local::diagnostic_game(id)?;
            let same = Path::new(g.game["directory"].as_str().unwrap_or_default())
                .canonicalize()
                .is_ok_and(|p| p == root);
            if !same {
                return Err("O perfil de mods pertence a outra instalação.".into());
            }
            super::nexus_profile::idle(&g)?;
            Some(g)
        }
        None => {
            let matches: Vec<_> = super::nexus_local::workspace()?
                .games
                .into_iter()
                .filter(|g| {
                    Path::new(g.game["directory"].as_str().unwrap_or_default())
                        .canonicalize()
                        .is_ok_and(|p| p == root)
                })
                .collect();
            if matches.len() > 1 {
                return Err("Há mais de um perfil de mods para esta instalação. Resolva a duplicação antes de redefinir.".into());
            }
            let game = matches.into_iter().next();
            if let Some(g) = &game {
                super::nexus_profile::idle(g)?;
            }
            game
        }
    };
    let mut next = game_reset::settings(target.advanced(), request.section);
    // Options go back to zero, but a mod that stays installed (DLSSNR) keeps the line it needs;
    // "All" removes those mods too.
    if request.section == Section::Options {
        next.launch_options = super::mod_launch_options::mod_launch_options_for(root.to_string_lossy().into_owned())?;
    }
    if request.section != Section::Mods {
        next.validate()?;
    }
    let mut completed = vec![];
    let mut warnings = vec![];
    let result:Result<(),String>=async{
 if matches!(request.section,Section::All|Section::Mods){
  if let Some(g)=&nexus {let id=g.id.clone();tauri::async_runtime::spawn_blocking(move||super::nexus_local::reset_game_mods(&id)).await.map_err(|e|e.to_string())??;completed.push("Mods gerenciados removidos; originais registrados restaurados e pacotes locais limpos".into());}
  else{completed.push("Nenhum perfil Nexus associado a esta instalação".into());}
  warnings.push("Arquivos de mods instalados fora do gerenciamento, sem registro de origem, podem permanecer. Verifique os arquivos pelo launcher de origem se necessário.".into());
 }
 if matches!(request.section,Section::All|Section::Dlss){
  let name=target.json()?["name"].as_str().unwrap_or_default().to_string();
  let legacy=crate::core::config_manager::load_config_result()?.and_then(|config|config.custom_game_paths.get(&name).cloned());
  let paths=super::frame_generation::resolve_game_mod_paths(root.to_string_lossy().into(),target.executable(),Some(target.key()),legacy).await?;
  let paths=serde_json::to_value(paths).map_err(|e|e.to_string())?;
  if let Some(destination)=paths["dlss_destination"].as_str(){
   let details=serde_json::to_value(super::installer::get_game_installation_details(destination.into()).await?).map_err(|e|e.to_string())?;
   if details["status"].as_str().is_some_and(|s|s!="Não Instalado"){
    super::installer::install_mod(app.clone(),destination.into(),"remove".into(),request.bitness.clone(),request.gpu_arch.clone(),None,None,String::new(),None,None,None,None,None).await?;
   }
   let current=next.launch_options.take();
   next.launch_options=if let Target::External(g)=&target{if g.launcher=="Steam"{if let Some(id)=&g.app_id{
    let sync=super::steam_launch_options::apply_steam_mod_launch_options(id.clone(),destination.into(),current,None,Some(g.external_id.clone()))?;
    serde_json::to_value(sync).map_err(|e|e.to_string())?["launch_options"].as_str().map(str::to_string)
   }else{return Err("Identificador Steam indisponível.".into());}}else{super::mod_launch_options::apply_mod_launch_options(destination.into(),current,None)?}}else{super::mod_launch_options::apply_mod_launch_options(destination.into(),current,None)?};
   completed.push("DLSSNR-AMD removido pelo desinstalador e vínculo de inicialização atualizado".into());
  }else{completed.push("Nenhum destino DLSSNR-AMD identificado".into());}
 }
 if matches!(request.section,Section::All|Section::FrameGeneration){super::frame_generation::reset_selection(&target.key())?;completed.push("MAKO e Overlay PeliGames desativados neste jogo; runtime compartilhado preservado".into());}
 if !matches!(request.section,Section::Mods){target=persist(target.clone(),next.clone()).await?;}
 if let Some(g)=&nexus{
  if matches!(request.section,Section::All|Section::Options){super::nexus_local::clear_game_launch_options(&g.id)?;}
  if !matches!(request.section,Section::Mods)&&!matches!(target,Target::Nexus(_)){super::nexus_launch_options::configure_nexus_advanced(g.id.clone(),next).await?;}
 }
 if matches!(request.section,Section::Performance|Section::Options|Section::Advanced|Section::All){completed.push("Configurações da seleção redefinidas; Proton, componentes Wine, benchmarks, visual do MangoHUD e backups preservados".into());}
 Ok(())
 }.await;
    if let Err(error) = result {
        crate::core::logger::event(
            "ERROR",
            &format!(
                "Redefinição {:?}: {error}; concluído: {}",
                request.section,
                completed.join("; ")
            ),
        );
        return Err(format!(
            "{error}{}",
            if completed.is_empty() {
                String::new()
            } else {
                format!("\nEtapas já concluídas: {}.", completed.join("; "))
            }
        ));
    }
    crate::core::logger::event(
        "INFO",
        &format!(
            "Redefinição {:?} concluída: {}",
            request.section,
            root.display()
        ),
    );
    Ok(ResultInfo {
        game: target.json()?,
        completed,
        warnings,
    })
}
