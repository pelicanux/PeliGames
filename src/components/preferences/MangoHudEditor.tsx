import { MangoHudPositionPicker } from "./MangoHudPositionPicker";
import { SelectionMenu } from "../SelectionMenu";
import { mangoHudGroups } from "./mangoHudGroups";
import { useEffect, useRef, useState } from "react";
import catalog from "../../config/mangohudOptions.json";
import { loadMangoHudProfile, saveMangoHudProfile, updateMangoHudPreview, mangoHudPreview, getMangoHudStatus, type MangoHudSettings } from "../../services/mangohud";

type Spec = {key:string;label:string;section:string;kind:string;default:boolean|number|string;min?:number;max?:number;step?:number;values?:string[]};
const options:Spec[] = catalog;
export const mangoHudDefaults = () => Object.fromEntries(options.map(o => [o.key,o.default])) as MangoHudSettings;
const pages = [{id:"profiles",label:"Perfis"},{id:"visual",label:"Aparência"},{id:"fps",label:"FPS e quadros"},{id:"metrics",label:"Métricas"},{id:"extras",label:"Extras e registros"}];
const groups = {visual:"Aparência do painel",fps:"FPS e sincronização",gpu:"GPU e memória gráfica",cpu:"CPU, memória e armazenamento",extras:"Sistema e informações adicionais",logs:"Registro de desempenho"};
const selectLabels:Record<string,Record<string,string>>={fps_color:{"E06C75,E5C07B,98C379":"Suave","F87171,FBBF24,4ADE80":"Vermelho, âmbar e verde","FF5555,FFCC55,55CCFF":"Vermelho, âmbar e azul"},position:{"top-left":"Superior esquerdo","top-center":"Superior central","top-right":"Superior direito","middle-left":"Centro esquerdo","middle-right":"Centro direito","bottom-left":"Inferior esquerdo","bottom-center":"Inferior central","bottom-right":"Inferior direito"},fps_metrics:{"":"Desativado","avg":"Média","avg,0.01":"Média e 1% mínimo","avg,0.01,0.001":"Média, 1% e 0,1% mínimo"},vsync:{"-1":"Padrão do jogo","0":"Adaptativo","1":"Desativado","2":"Mailbox","3":"Ativado"},gl_vsync:{"-2":"Padrão do jogo","0":"Desativado","1":"A cada quadro","2":"A cada 2 quadros","3":"A cada 3 quadros","4":"A cada 4 quadros"},gpu_list:{"":"Seleção automática","0":"GPU 0","1":"GPU 1","0,1":"GPU 0 e GPU 1"},fps_limit_method:{late:"Após apresentar o quadro",early:"Antes de apresentar o quadro"}};
export function MangoHudEditor({ onDirtyChange, onInstall }: { onDirtyChange:(dirty:boolean)=>void; onInstall:()=>void }) {
  const [settings,setSettings]=useState<MangoHudSettings>(mangoHudDefaults),[saved,setSaved]=useState("");
  const [page,setPage]=useState("profiles"),[path,setPath]=useState("");
  const [ready,setReady]=useState(false),[saving,setSaving]=useState(false),[previewBusy,setPreviewBusy]=useState(false),[running,setRunning]=useState(false);
  const [error,setError]=useState(""),[notice,setNotice]=useState("");
  const mounted=useRef(false),previewLock=useRef(false),saveLock=useRef(false),previewQueue=useRef(Promise.resolve());
  const [saveError,setSaveError]=useState("");
  const [checkingGlobal,setCheckingGlobal]=useState(false),[missingGlobal,setMissingGlobal]=useState(false);
  const globalLock=useRef(false);
  const readyRef=useRef(ready),savedRef=useRef(saved);readyRef.current=ready;savedRef.current=saved;
  const latest=useRef(settings);latest.current=settings;
  const dirty=ready && JSON.stringify(settings)!==saved;
  useEffect(()=>{onDirtyChange(dirty || saving || previewBusy || checkingGlobal);return()=>onDirtyChange(false);},[dirty,saving,previewBusy,checkingGlobal,onDirtyChange]);
  const load = async () => { setError("");try {const p=await loadMangoHudProfile();if(mounted.current){const next={...mangoHudDefaults(),...p.settings};setSettings(next);setSaved(JSON.stringify(next));setPath(p.config_path);setReady(true);}}catch(e){if(mounted.current)setError(String(e));} };
  useEffect(()=>{mounted.current=true;void load();return()=>{mounted.current=false;
    if(readyRef.current&&JSON.stringify(latest.current)!==savedRef.current)void saveMangoHudProfile(latest.current).catch(()=>{});
    void mangoHudPreview("stop").catch(()=>{});
  };},[]);
  useEffect(()=>{
    if(!running)return;
    const timer=setTimeout(()=>{const snapshot=settings;previewQueue.current=previewQueue.current.catch(()=>{}).then(async()=>{if(!mounted.current)return;await updateMangoHudPreview(snapshot);}).catch(e=>{if(mounted.current)setError(String(e));});},180);
    return()=>clearTimeout(timer);
  },[settings,running]);
  useEffect(()=>{if(!running)return;const timer=setInterval(()=>{void mangoHudPreview("status").then(s=>{if(mounted.current&&!s.running)setRunning(false);}).catch(e=>{if(mounted.current){setError(String(e));setRunning(false);}});},1500);return()=>clearInterval(timer);},[running]);
  const change=(key:string,value:boolean|number|string)=>{setSettings(s=>({...s,[key]:value}));setNotice("");setSaveError("");};
  const save=async()=>{
    if(saveLock.current)return;
    saveLock.current=true;setSaving(true);setSaveError("");
    const snapshot=latest.current;
    const task=saveMangoHudProfile(snapshot);
    try{const p=await task;if(mounted.current){setSaved(JSON.stringify(snapshot));setPath(p);}}
    catch(e){if(mounted.current)setSaveError(e instanceof Error?e.message:String(e));}
    finally{saveLock.current=false;if(mounted.current)setSaving(false);}
  };
  useEffect(()=>{
    if(!ready||!dirty||saving||saveError)return;
    const timer=setTimeout(()=>void save(),500);
    return()=>clearTimeout(timer);
  },[settings,ready,dirty,saving,saveError]);
  const preview=async()=>{if(previewLock.current)return;previewLock.current=true;setPreviewBusy(true);setError("");try{await previewQueue.current;const s=await mangoHudPreview(running?"stop":"start",latest.current);if(mounted.current)setRunning(s.running);}catch(e){if(mounted.current)setError(String(e));}finally{previewLock.current=false;if(mounted.current)setPreviewBusy(false);}};
  const toggleGlobal=async()=>{
    if(globalLock.current)return;
    if(settings.launcher_enabled){change("launcher_enabled",false);setMissingGlobal(false);return;}
    globalLock.current=true;setCheckingGlobal(true);setMissingGlobal(false);setError("");
    try{const status=await getMangoHudStatus();if(!mounted.current)return;
      if(status.installed){setSettings(s=>({...s,launcher_enabled:true}));setSaveError("");}
      else setMissingGlobal(true);
    }catch(e){if(mounted.current)setError(e instanceof Error?e.message:String(e));}
    finally{globalLock.current=false;if(mounted.current)setCheckingGlobal(false);}
  };
  const preset=(kind:string)=>{const next=mangoHudDefaults();next.launcher_enabled=settings.launcher_enabled;for(const o of options){if(o.kind==="toggle"&&["fps","gpu","cpu","extras"].includes(o.section))next[o.key]=false;}next.fps=true;if(kind!=="fps"){for(const k of ["gpu_stats","cpu_stats","ram","vram","frametime"])next[k]=true;}if(kind==="full"){for(const k of ["gpu_temp","cpu_temp","gpu_core_clock","gpu_power","cpu_mhz","frame_timing","resolution","display_server","wine"])next[k]=true;}next.horizontal=kind==="horizontal";if(kind==="fps"){next.fps_metrics="";next.custom_text_center="";}setSettings(next);setNotice("");setSaveError("");};
  const palette=(colors:string[])=>{setSettings(s=>({...s,text_color:colors[0],gpu_color:colors[1],cpu_color:colors[2],ram_color:colors[3],vram_color:colors[4],frametime_color:colors[1]}));setNotice("");setSaveError("");};
  const renderControl=(o:Spec)=><div key={o.key} className={`mango-control mango-control-${o.kind}${o.key==="position"?" mango-position-control":""}`}><span>{o.label}</span>{o.key==="position"?<MangoHudPositionPicker value={String(settings.position)} onChange={value=>change("position",value)} />:o.kind==="toggle"?<button type="button" role="switch" aria-label={o.label} aria-checked={Boolean(settings[o.key])} className={`advanced-toggle ${settings[o.key]?"enabled":""}`} onClick={()=>change(o.key,!settings[o.key])}><span/></button>:o.kind==="select"?<SelectionMenu label={o.label} value={String(settings[o.key])} onChange={value=>change(o.key,value)} options={(o.values??[]).map(v=>({value:v,label:selectLabels[o.key]?.[v]??v}))} />:o.kind==="color"?<input aria-label={o.label} type="color" value={`#${settings[o.key]}`} onChange={e=>change(o.key,e.target.value.slice(1).toUpperCase())}/>:o.kind==="text"?<input aria-label={o.label} maxLength={64} value={String(settings[o.key])} onChange={e=>change(o.key,e.target.value.replace(/[\x00-\x1f\x7f]/g,""))}/>:<div className="mango-number"><input aria-label={o.label} type="number" min={o.min} max={o.max} step={o.step} value={Number(settings[o.key])} onChange={e=>{const n=e.target.valueAsNumber;if(Number.isFinite(n))change(o.key,Math.min(o.max??1000,Math.max(o.min??0,n)));}}/>{["background_alpha","alpha","round_corners","font_size"].includes(o.key)&&<input aria-label={`${o.label}: ajuste`} type="range" min={o.min} max={o.max} step={o.step} value={Number(settings[o.key])} onChange={e=>change(o.key,Number(e.target.value))}/>}</div>}</div>;
  const renderGroup=(id:keyof typeof groups)=><div key={id} className="mango-controls">
    <h3>{groups[id]}</h3>
    <div className="mango-subcategories">{mangoHudGroups[id].map(group=><section key={group.title} className="preference-panel mango-subcategory" aria-label={`${groups[id]} · ${group.title}`}>
      <h4>{group.title}</h4><div className="mango-control-grid">{group.keys.map(key=>options.find(o=>o.key===key)!).map(renderControl)}</div>
    </section>)}</div>
    {id==="gpu"&&<p>Sensores e métricas disponíveis dependem da GPU e do driver. Temperatura e frequência da memória exigem VRAM ativada.</p>}
    {id==="logs"&&<p>Os registros ficam na pasta MangoHUD/logs do PeliGames. O envio de registros para serviços externos permanece desativado.</p>}
  </div>;
  return <div className="mango-editor">
    <div className="mango-editor-heading"><h3>Seu painel de desempenho</h3><p>Um perfil exclusivo do PeliGames, aplicado somente aos jogos com MangoHUD ativado.</p></div>
    <section className="preference-panel mango-global-panel">
      <div className="mango-control"><span><strong>Ativar MangoHUD por padrão</strong><small>Mostrar o monitoramento em todos os jogos iniciados pelo PeliGames, em qualquer biblioteca.</small></span><button type="button" role="switch" aria-label="Ativar MangoHUD por padrão" aria-checked={Boolean(settings.launcher_enabled)} disabled={!ready||checkingGlobal} className={`advanced-toggle ${settings.launcher_enabled?"enabled":""}`} onClick={()=>void toggleGlobal()}><span/></button></div>
      {checkingGlobal&&<p role="status">Verificando MangoHUD…</p>}
      {missingGlobal&&<p role="alert">MangoHUD não foi detectado. <button type="button" className="mango-global-install" onClick={onInstall}>Clique aqui para instalar</button></p>}
    </section>
    <nav className="mango-tabs" aria-label="Configuração do MangoHUD">{pages.map(p=><button type="button" key={p.id} aria-pressed={page===p.id} onClick={()=>setPage(p.id)}>{p.label}</button>)}</nav>
    {!ready?<section className="preference-panel"><p>{error||"Carregando configuração…"}</p>{error&&<button type="button" onClick={()=>void load()}>Tentar novamente</button>}</section>:<fieldset className="mango-editor-content" disabled={previewBusy}>
      {page==="profiles"&&<><section className="preference-panel"><h3>Ponto de partida</h3><p>Escolha um conjunto de informações e ajuste as opções nas outras abas.</p><div className="mango-presets">{[{id:"basic",name:"Essencial",description:"FPS, uso de CPU e GPU e memória."},{id:"full",name:"Detalhado",description:"Sensores, frequências e gráfico de quadros."},{id:"horizontal",name:"Faixa horizontal",description:"Informações essenciais lado a lado."},{id:"fps",name:"Somente FPS",description:"Um painel discreto com a taxa de quadros."}].map(p=><button type="button" key={p.id} onClick={()=>preset(p.id)}><strong>{p.name}</strong><small>{p.description}</small></button>)}</div></section><section className="preference-panel"><h3>Cores do monitoramento</h3><div className="mango-palettes">{[{name:"PeliGames",colors:["FFFFFF","54C58D","55B9F3","DCA0D5","B793EF"]},{name:"Terminal",colors:["9BEDB0","41D77A","67C993","8FDF9F","56AC7E"]},{name:"Monocromático",colors:["FFFFFF","FFFFFF","FFFFFF","FFFFFF","FFFFFF"]},{name:"Âmbar",colors:["FFE5B0","F4B350","FFD787","E7A25D","C99568"]}].map(p=><button type="button" key={p.name} onClick={()=>palette(p.colors)}><span aria-hidden="true">{p.colors.slice(1).map((c,i)=><i key={i} style={{background:`#${c}`}}/>)}</span>{p.name}</button>)}</div></section></>}
      {page==="visual"&&renderGroup("visual")}{page==="fps"&&renderGroup("fps")}{page==="metrics"&&<>{renderGroup("gpu")}{renderGroup("cpu")}</>}{page==="extras"&&<>{renderGroup("extras")}{renderGroup("logs")}</>}
    </fieldset>}
    {error&&ready&&<p role="alert" className="preferences-error">{error}</p>}{notice&&<p role="status">{notice}</p>}
    {saveError&&<div role="alert" className="preferences-error"><p>Não foi possível salvar automaticamente: {saveError}</p><div className="monitoring-actions"><button type="button" disabled={saving} onClick={()=>setSaveError("")}>Tentar novamente</button><button type="button" disabled={saving} onClick={()=>{setSettings(JSON.parse(saved));setSaveError("");}}>Desfazer alterações</button></div></div>}
    <div className="mango-editor-footer"><div><span role="status">{saveError?"Falha ao salvar":saving?"Salvando automaticamente…":dirty?"Aguardando salvamento…":"Alterações salvas automaticamente"}</span>{path&&<small>{path}</small>}{running&&<small>Preview externa aberta · atualização ao vivo</small>}</div><div className="monitoring-actions"><button type="button" disabled={!ready||previewBusy} onClick={()=>void preview()}>{previewBusy?"Aguarde…":running?"Fechar preview":"Abrir preview"}</button></div></div>
  </div>;
}
