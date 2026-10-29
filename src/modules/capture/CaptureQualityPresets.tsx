import type {AudioSelection} from "./CaptureAudioFields";
const standardPreset = {id:"standard",label:"Padrão",description:"Equilíbrio para começar a gravar.",detail:"720p · 60 FPS",settings:{resolution:"1280x720",fps:60,rate_control:"qp",quality_preset:"high",codec:"auto"}} as const;
export const captureQualityPresets = [
 {id:"low",label:"Baixa qualidade",description:"Economiza espaço para clipes rápidos.",detail:"480p · 30 FPS",settings:{resolution:"854x480",fps:30,rate_control:"qp",quality_preset:"medium",codec:"auto"}},
 standardPreset,
 {id:"high",label:"Alta qualidade",description:"Mais definição, com arquivos maiores.",detail:"1080p · 60 FPS",settings:{resolution:"1920x1080",fps:60,rate_control:"qp",quality_preset:"very_high",codec:"auto"}},
] as const;
export const defaultCaptureQualitySettings = standardPreset.settings;
export function matchingCapturePreset(settings:AudioSelection){
 return captureQualityPresets.find(preset=>Object.entries(preset.settings).every(([key,value])=>settings[key as keyof AudioSelection]===value))?.id||"custom";
}
export function CaptureQualityPresets({settings,disabled,custom,onChange,onCustom}:{settings:AudioSelection;disabled:boolean;custom:boolean;onChange:(settings:Partial<AudioSelection>)=>void;onCustom:()=>void}){
 const selected=custom?"custom":matchingCapturePreset(settings);
 return <div className="capture-quality-presets"><h4>Qualidade da gravação</h4><p>Escolha um ponto de partida ou ajuste os controles abaixo. Os presets preservam o formato, a fonte e o áudio escolhidos.</p>
 <div className="capture-preset-options" role="group" aria-label="Presets de qualidade">
 {captureQualityPresets.map(preset=><button key={preset.id} type="button" disabled={disabled} aria-pressed={selected===preset.id} onClick={()=>onChange(preset.settings)}><span><strong>{preset.label}</strong><i aria-hidden="true">{selected===preset.id?"✓":""}</i></span><small>{preset.description}</small><em>{preset.detail}</em></button>)}
 <button type="button" disabled={disabled} aria-pressed={selected==="custom"} onClick={onCustom}><span><strong>Personalizado</strong><i aria-hidden="true">{selected==="custom"?"✓":""}</i></span><small>Defina resolução, FPS, codec e qualidade.</small><em>Seus próprios ajustes</em></button>
 </div><p>O codec automático escolhe uma opção compatível com a GPU. O tamanho dos arquivos varia com o conteúdo; a resolução preserva a proporção da imagem.</p></div>;
}
