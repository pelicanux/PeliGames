import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useGameSaveStatus,GameSaveStatusIndicator} from "../../components/GameSaveStatus";
import {DebouncedSave} from "../../services/debouncedSave";
import {openFrameGenerationPreferences} from "../../services/frameGenerationNavigation";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import type {GameInfo} from "../../components/GameGrid";
import {defaultAfmf,isFrameGenerationWrapper,normalizeAdvanced,type AdvancedSettings,type AfmfSettings} from "../../services/advancedSettings";
import {updatePeliGamesSettings} from "../../services/installedLibrary";
import {useI18n} from "../../i18n/I18nContext";
import "./frameGeneration.css";

type Runtime={installed_versions:{version:string;current:boolean;directory:string}[];current_version:string;install_version:string;available_version:string;update_available:boolean;update_compatible:boolean;install_directory:string};
type Support={available:boolean;reason:string};
type Choice<T extends string>={value:T;pt:string;en:string};

const MODES:Choice<AfmfSettings["performance_mode"]>[]=[{value:"auto",pt:"Automático",en:"Automatic"},{value:"quality",pt:"Qualidade",en:"Quality"},{value:"performance",pt:"Desempenho",en:"Performance"}];
const SEARCH:Choice<AfmfSettings["search_mode"]>[]=[{value:"auto",pt:"Automática",en:"Automatic"},{value:"standard",pt:"Padrão",en:"Standard"},{value:"high",pt:"Alta",en:"High"}];
const MOTION:Choice<AfmfSettings["fast_motion"]>[]=[{value:"blend",pt:"Misturar",en:"Blend"},{value:"repeat",pt:"Repetir quadro",en:"Repeat frame"}];

/** AFMF for one game: a guided setup (install → enable → relaunch) and its options, saved automatically. */
export function AfmfPanel({game,disabled,running=false,onSaved,onPersist}:{game:GameInfo;disabled:boolean;running?:boolean;onSaved:(game:GameInfo)=>void;onPersist?:(settings:AdvancedSettings)=>Promise<void>}){
 const pt=useI18n().language==="pt";
 const advanced=normalizeAdvanced(game.advanced);
 const [settings,setSettings]=useState<AfmfSettings>(()=>advanced.afmf??defaultAfmf);
 const [runtime,setRuntime]=useState<Runtime>();const [support,setSupport]=useState<Support>();
 const [busy,setBusy]=useState(false);const [error,setError]=useState("");
 const [saveStatus,setSaveStatus]=useState<""|"pending"|"saving"|"saved"|"error">("");const [savePanel,setSavePanel]=useState<"afmf-prepare"|"afmf-options">("afmf-prepare");
 const headerStatus=useGameSaveStatus(saveStatus,savePanel);
 const autosave=useRef(new DebouncedSave());const lock=useRef(false);
 useEffect(()=>()=>{void autosave.current.flush();},[]);
 const key=game.external_id||game.path;
 useEffect(()=>{setSettings(normalizeAdvanced(game.advanced).afmf??defaultAfmf)},[key]);
 const loadRuntime=()=>invoke<Runtime>("frame_generation_state",{gameKey:"runtime-preferences:afmf",backend:"afmf",activeProfile:null}).then(setRuntime).catch(e=>setError(String(e)));
 useEffect(()=>{void loadRuntime();const reload=()=>void loadRuntime();window.addEventListener("frameGenerationRuntimeChanged",reload);return()=>window.removeEventListener("frameGenerationRuntimeChanged",reload)},[]);
 useEffect(()=>{let live=true;setSupport(undefined);void invoke<Support>("afmf_support",{directory:game.directory||game.path,executable:game.executable||null,flatpak:Boolean(game.flatpak)}).then(s=>{if(live)setSupport(s)}).catch(e=>{if(live)setSupport({available:false,reason:String(e)})});return()=>{live=false}},[key,game.directory,game.executable,game.flatpak]);

 const installed=Boolean(runtime?.installed_versions.some(r=>r.current));
 const supported=Boolean(onPersist)||game.launcher==="PeliGames"||Boolean(game.external_id);
 const makoOn=(advanced.wrappers??[]).some(w=>isFrameGenerationWrapper(w)&&w.enabled);
 const conflict=makoOn?(pt?"O MAKO está ativado neste jogo. Desative-o na aba MAKO antes de ativar o AFMF: os dois geradores não funcionam juntos.":"MAKO is enabled for this game. Disable it in the MAKO tab first: the two generators cannot run together."):(advanced.options.mlfg||advanced.options.optiscaler)?(pt?"Desative a geração de quadros por ML e o OptiScaler na aba Avançado antes de ativar o AFMF.":"Disable ML frame generation and OptiScaler in the Advanced tab before enabling AFMF."):"";
 const blocked=busy||disabled||running;

 const persist=async(next:AfmfSettings)=>{if(lock.current)return;lock.current=true;setBusy(true);setSaveStatus("saving");try{
  const nextAdvanced={...advanced,afmf:next};
  const updated=onPersist?await onPersist(nextAdvanced).then(()=>({...game,advanced:nextAdvanced})):game.external_id?await invoke<GameInfo>("configure_external_game",{settings:{external_id:game.external_id,advanced:nextAdvanced}}):await updatePeliGamesSettings({path:game.path,name:game.name,executable:game.executable||game.path,proton:game.proton||"",prefix:game.prefix,advanced:nextAdvanced});
  onSaved({...game,...updated,library_view:game.library_view});window.dispatchEvent(new Event("peligamesLibraryChanged"));setSaveStatus("saved");setError("");
 }catch(e){setSaveStatus("error");setError(String(e))}finally{lock.current=false;setBusy(false)}};
 const patch=(value:Partial<AfmfSettings>,panel:"afmf-prepare"|"afmf-options"="afmf-options")=>{const next={...settings,...value};setSettings(next);setSavePanel(panel);setSaveStatus("pending");setError("");autosave.current.schedule(()=>persist(next));};

 const segments=<T extends string>(label:string,choices:Choice<T>[],value:T,onPick:(v:T)=>void)=><div className="modern-dlss-segments fg-multipliers" role="group" aria-label={label}>{choices.map(c=><button type="button" key={c.value} aria-pressed={value===c.value} disabled={blocked} onClick={()=>onPick(c.value)}>{pt?c.pt:c.en}</button>)}</div>;
 const canEnable=installed&&supported&&Boolean(support?.available)&&!conflict;

 return <section className="frame-generation-panel fg-game-workflow afmf-panel">
  <div className="modern-dlss-panels fg-game-grid">
   <section className="modern-dlss-card fg-game-setup" aria-label={pt?"Preparar o AFMF":"Prepare AFMF"}>
    <div className="modern-panel-heading"><h3>{pt?"Preparar para este jogo":"Prepare for this game"}</h3><GameSaveStatusIndicator scope="afmf-prepare"/></div>
    <p className="modern-dlss-description">{pt?"O AFMF gera um quadro intermediário para cada quadro real (2×), a partir da imagem do jogo. Siga os passos abaixo; as alterações são salvas automaticamente.":"AFMF generates one in-between frame per real frame (2×) from the game's image. Follow the steps below; changes save automatically."}</p>

    <div className="afmf-steps">
     {!installed && <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{pt?"Prepare o componente":"Prepare the component"}</h4><p>{pt?"Instale o AFMF em Configurações → Complementos. A instalação é compartilhada entre os jogos.":"Install AFMF in Settings → Add-ons. The installation is shared across games."}</p></div></div>
      {runtime ? <div className="fg-component-row"><button type="button" className="modern-mini-btn" disabled={blocked} onClick={()=>openFrameGenerationPreferences("afmf")}>{pt?"Instalar":"Install"}</button></div> : <p role="status">{pt?"Verificando componente…":"Checking component…"}</p>}
     </div>}

     <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{pt?"Ative neste jogo":"Enable for this game"}</h4><p>{support?.reason??(pt?"Verificando a compatibilidade do jogo…":"Checking the game's compatibility…")}</p></div></div>
      <div className="fg-toolbar"><div className="fg-master-toggle"><span>{pt?"Ativar AFMF":"Enable AFMF"}</span><ModOptionSwitch label={pt?"Ativar AFMF neste jogo":"Enable AFMF for this game"} enabled={settings.enabled} disabled={blocked||(!settings.enabled&&!canEnable)} onToggle={()=>patch({enabled:!settings.enabled},"afmf-prepare")}/></div><span className="fg-generator-name">AFMF · 2×</span></div>
      {!installed&&runtime&&<p className="mod-step-note">{pt?"Instale o AFMF em Configurações → Complementos para liberar a ativação.":"Install AFMF in Settings → Add-ons to unlock activation."}</p>}
      {conflict&&<p className="fg-partition-note" role="alert">{conflict}</p>}
      {!supported&&<p className="mod-step-note">{pt?"Registre esta instalação na biblioteca principal para configurar sua execução.":"Register this installation in the main library to configure its launch."}</p>}
     </div>

     <div className="afmf-step">
      <div className="mod-step-heading"><div><h4>{pt?"Abra (ou reabra) o jogo":"Launch (or relaunch) the game"}</h4><p>{pt?"O AFMF lê as opções quando o jogo inicia. Qualquer mudança vale na próxima abertura.":"AFMF reads its options when the game starts. Any change applies on the next launch."}</p></div></div>
      <p className="mod-step-note">{pt?"Com o Overlay PeliGames ligado (aba Desempenho), abra o painel pelo atalho configurado para consultar os ajustes de AFMF enviados ao jogo. Ele não muda as opções durante a partida.":"With the PeliGames overlay on (Performance tab), open the in-game panel with your configured shortcut to view the AFMF settings sent to the game. It cannot change them during play."}</p>
      {game.steam_pending&&<p className="mod-step-note">{pt?"Há alterações pendentes na Steam. Aplique-as junto ao botão Jogar.":"Steam has pending changes. Apply them beside the Play button."}</p>}
      {running&&<p role="status">{pt?"Jogo em execução: feche-o para alterar o AFMF.":"Game running: close it to change AFMF."}</p>}
     </div>
    </div>

    {!headerStatus&&<span role="status" className="fg-autosave-status">{saveStatus==="saving"?(pt?"Salvando…":"Saving…"):saveStatus==="error"?(pt?"Não foi possível salvar.":"Could not save."):saveStatus==="saved"?(pt?"Salvo automaticamente":"Saved automatically"):(pt?"As alterações são salvas automaticamente":"Changes save automatically")}</span>}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
   </section>

   <section className="modern-dlss-card fg-game-options" aria-label={pt?"Opções do AFMF":"AFMF options"}>
    <div className="modern-panel-heading"><h3>{pt?"Opções":"Options"}</h3><GameSaveStatusIndicator scope="afmf-options"/></div>
    <p className="modern-dlss-description">{pt?"Os padrões funcionam na maioria dos jogos. Ajuste se notar artefatos ou queda de desempenho.":"Defaults work for most games. Adjust if you notice artifacts or a performance drop."}</p>
    <div className="fg-controls">
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Modo":"Mode"}</span><small>{pt?"Qualidade busca menos artefatos; Desempenho custa menos à GPU.":"Quality reduces artifacts; Performance costs the GPU less."}</small></span>{segments(pt?"Modo":"Mode",MODES,settings.performance_mode,v=>patch({performance_mode:v}))}</div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Busca de movimento":"Motion search"}</span><small>{pt?"Alta acompanha movimentos amplos, com mais custo.":"High follows wide motion, at a higher cost."}</small></span>{segments(pt?"Busca de movimento":"Motion search",SEARCH,settings.search_mode,v=>patch({search_mode:v}))}</div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Movimento rápido":"Fast motion"}</span><small>{pt?"Quando o movimento é rápido demais: misturar suaviza, repetir evita deformações.":"When motion is too fast: blend smooths, repeat avoids warping."}</small></span>{segments(pt?"Movimento rápido":"Fast motion",MOTION,settings.fast_motion,v=>patch({fast_motion:v}))}</div>
     <label className="modern-dlss-setting fg-control-field fg-setting"><span className="fg-setting-copy"><span>{pt?"FPS mínimo para gerar":"Minimum FPS to generate"}</span><small>{pt?"Abaixo deste FPS real nenhum quadro é gerado. 0 gera sempre.":"Below this real FPS no frame is generated. 0 always generates."}</small></span><input type="number" min={0} max={240} value={settings.min_fps} disabled={blocked} onChange={e=>{const v=Math.max(0,Math.min(240,Math.round(Number(e.target.value)||0)));patch({min_fps:v})}}/></label>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Detectar HUD":"Detect HUD"}</span><small>{pt?"Mantém menus e HUD fixos, sem distorção.":"Keeps menus and HUD still, without warping."}</small></span><ModOptionSwitch label={pt?"Detectar HUD":"Detect HUD"} enabled={settings.hud_detect} disabled={blocked} onToggle={()=>patch({hud_detect:!settings.hud_detect})}/></div>
     <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{pt?"Ritmo dos quadros":"Frame pacing"}</span><small>{pt?"Distribui os quadros de forma uniforme; segura um pouco cada quadro real.":"Spaces frames evenly; holds each real frame slightly."}</small></span><ModOptionSwitch label={pt?"Ritmo dos quadros":"Frame pacing"} enabled={settings.pacing} disabled={blocked} onToggle={()=>patch({pacing:!settings.pacing})}/></div>
    </div>
    {advanced.gamescope?.enable_upscaling||advanced.gamescope?.enable_limiter||advanced.gamescope?.force_grab_cursor||advanced.gamescope?.additional_options?.trim()?<p className="fg-overlay-support-note">{pt?"Gamescope detectado: o modo compatível do AFMF é ativado automaticamente.":"Gamescope detected: AFMF's compatible mode is enabled automatically."}</p>:null}
    <div className="fg-compatibility"><strong>{pt?"Compatibilidade":"Compatibility"}</strong>
     <p>{pt?"Testado em placas AMD RX 7000 e RX 9000 com o driver RADV. Outras placas não têm teste oficial. Exige Vulkan 1.1 ou mais novo, direto ou pelo DXVK/VKD3D do Proton.":"Tested on AMD RX 7000 and RX 9000 cards with the RADV driver. Other cards are untested. Requires Vulkan 1.1 or newer, directly or through Proton's DXVK/VKD3D."}</p>
     <p>{pt?"Multiplicador fixo em 2×. Se o jogo já roda na taxa do monitor com V-Sync, não há quadros gerados. Objetos finos e rápidos podem sumir dos quadros intermediários.":"Fixed 2× multiplier. A game already at the monitor's refresh rate with V-Sync gets no generated frames. Thin, fast objects may vanish from in-between frames."}</p>
     <p>{pt?"Não use junto com MAKO, lsfg-vk ou a geração de quadros do OptiScaler. O MangoHUD pode contar só os quadros reais.":"Do not combine with MAKO, lsfg-vk or OptiScaler frame generation. MangoHUD may count only real frames."}</p>
    </div>
   </section>
  </div>
 </section>;
}
