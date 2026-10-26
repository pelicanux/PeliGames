import type {ReactNode} from "react";
import {SelectionMenu} from "../../components/SelectionMenu";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import {gpuChoices,type LinuxGpu} from "../../hooks/useLinuxGpus";
import type {FrameSettings} from "./FrameGenerationPanel";
export function MakoAdditionalControls({settings:s,gpus,gpuError,startupBlocked,liveBlocked,onChange,generationControls}:{
  settings:FrameSettings;gpus:LinuxGpu[];gpuError:string;startupBlocked:boolean;liveBlocked:boolean;onChange:(value:Partial<FrameSettings>)=>void;generationControls?:ReactNode;
}) {
  const toggle=(key:"allow_fp16"|"ultra_performance"|"live_enabled"|"scaling_enabled",label:string,help:string,defaultValue:boolean,blocked:boolean)=>
    <div className="modern-dlss-option fg-setting"><span className="fg-setting-copy"><span>{label}</span><small>{help}</small></span><ModOptionSwitch label={label} enabled={s[key]??defaultValue} disabled={blocked} onToggle={()=>onChange({[key]:!(s[key]??defaultValue)})}/></div>;
  const number=(key:"base_fps_cap"|"refresh_threshold"|"scaling_factor"|"scaling_sharpness",label:string,help:string,min:number,max:number,step:number,fallback:number,blocked:boolean)=>
    <label className="modern-dlss-setting fg-control-field fg-setting"><span className="fg-setting-copy"><span>{label}</span><small>{help}</small></span><input type="number" aria-label={label} min={min} max={max} step={step} value={s[key]??fallback} disabled={blocked} onChange={e=>{const value=e.target.valueAsNumber;if(Number.isFinite(value))onChange({[key]:Number(Math.min(max,Math.max(min,Math.round(value/step)*step)).toFixed(2))})}}/></label>;
  return <div className="fg-additional-controls fg-options-columns">
    <div className="fg-options-column fg-controls">
    <h4>Geração de quadros</h4>
    {generationControls}
    {number("base_fps_cap","Limite de FPS base","Limita os quadros reais antes da geração. 0 desativa. Evite combinar com outros limitadores.",0,1000,1,0,liveBlocked)}
    {number("refresh_threshold","Limiar de atualização (Hz)","Pausa a geração quando a taxa confirmada pelo Gamescope é igual ou inferior a este valor. 0 desativa; depende do Gamescope.",0,1000,1,0,liveBlocked)}
    {toggle("ultra_performance","Ultra Performance","Preset mais leve: escala de movimento em 70%, modelo leve e LS1 Performance quando aplicável. Exige reiniciar.",false,startupBlocked)}
    {toggle("live_enabled","Geração ativa (Live FG Switch)","Pausa ou retoma a geração sem remover o MAKO. Durante a partida, exige que o jogo tenha iniciado com MAKO ativado.",true,liveBlocked)}
    </div><div className="fg-options-column fg-controls">
    <h4>GPU e precisão</h4>
    <div className="fg-setting fg-device-setting"><span className="fg-setting-copy"><span>GPU do MAKO</span><small>Use a mesma GPU que renderiza o jogo. Geração entre GPUs diferentes não é suportada. Exige reiniciar o jogo.</small></span><SelectionMenu label="GPU do MAKO" value={s.gpu??""} options={gpuChoices(gpus,s.gpu??"")} disabled={startupBlocked} onChange={gpu=>onChange({gpu})}/></div>
    {gpuError&&<p role="status">{gpuError}</p>}
    {toggle("allow_fp16","Aceleração FP16","Usa precisão reduzida nos modelos compatíveis. LS1 mantém FP32. Exige reiniciar.",true,startupBlocked)}
    <fieldset className="fg-scaling-group"><legend>Escala de imagem</legend>
    {toggle("scaling_enabled","Ativar escala de imagem","Prepara o escalonamento ao iniciar o jogo. Exige reiniciar; o fator efetivo depende da superfície e da resolução de saída.",false,startupBlocked)}
    <div className="fg-scaling-fields"><div className="fg-setting fg-device-setting"><span className="fg-setting-copy"><span>Método de escala</span></span><SelectionMenu label="Método de escala" value={s.scaling_method??"native"} disabled={startupBlocked||!s.scaling_enabled} options={[{value:"native",label:"Nativo · sem redimensionamento"},{value:"mako",label:"MAKO Scaler · passagem única"},{value:"ls1",label:"LS1 · qualidade"},{value:"ls1-performance",label:"LS1 · desempenho"}]} onChange={scaling_method=>onChange({scaling_method})}/></div>
    {number("scaling_factor","Fator de escala","Relação entre saída e imagem original (1–2×). Exige reiniciar.",1,2,.05,1.5,startupBlocked||!s.scaling_enabled||s.scaling_method==="native"||!s.scaling_method)}
    {number("scaling_sharpness","Nitidez","Intensidade de nitidez (0–1). Exige reiniciar.",0,1,.05,.8,startupBlocked||!s.scaling_enabled||s.scaling_method==="native"||!s.scaling_method)}
    </div></fieldset>
    </div>
  </div>;
}
