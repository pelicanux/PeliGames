import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {SelectionMenu} from "../../components/SelectionMenu";
export type AudioSelection={format:string;codec:string;resolution:string;fps:number;rate_control:string;bitrate_kbps:number;quality_preset:string;desktop_audio:boolean;microphone:boolean;microphone_noise_reduction:boolean;output_source:string;input_source:string};
type Encoder={vendor:string;encoder:string;gpu:string;card_path:string;pci_address:string;codecs:string[];containers:string[]};
type Source={id:string;label:string};type Sources={outputs:Source[];inputs:Source[];applications:Source[]};
export function CaptureAudioFields({settings,disabled,onChange,compact=false,section="all"}:{settings:AudioSelection;disabled:boolean;onChange:(value:Partial<AudioSelection>)=>void;compact?:boolean;section?:"all"|"video"|"audio"}){
 const [sources,setSources]=useState<Sources>({outputs:[{id:"default_output",label:"Saída padrão do sistema"}],inputs:[{id:"default_input",label:"Microfone padrão"}],applications:[]}),[error,setError]=useState(""),[busy,setBusy]=useState(false);
 const [encoder,setEncoder]=useState<Encoder>(),[codecError,setCodecError]=useState("");
 const mounted=useRef(true),pending=useRef(false);
 useEffect(()=>{mounted.current=true;if(section!=="audio")void loadEncoder();return()=>{mounted.current=false};},[section]);
 const loadEncoder=async()=>{try{const value=await invoke<Encoder>("capture_encoder_support");if(mounted.current){setEncoder(value);setCodecError("");}}catch(e){if(mounted.current)setCodecError(String(e));}};
 const refresh=async()=>{if(pending.current)return;pending.current=true;setBusy(true);setError("");try{const value=await invoke<Sources>("capture_audio_sources");if(mounted.current)setSources(value);}catch(e){if(mounted.current)setError(String(e));}finally{pending.current=false;if(mounted.current)setBusy(false);}};
 const options=(items:Source[],value:string)=>{const all=items.map(source=>({value:source.id,label:source.label}));if(!all.some(item=>item.value===value))all.push({value,label:`${value} · indisponível`});return all};
 return <>
 {section!=="audio"&&<>
 <div className="capture-video-fields">
 <div className="capture-preview-field"><span>Formato do vídeo</span><SelectionMenu label="Formato do vídeo" disabled={disabled} value={settings.format} onChange={format=>onChange({format,codec:format==="webm"&&["h264","hevc"].includes(settings.codec)?"auto":settings.codec})} options={[{value:"mp4",label:"MP4"},{value:"mkv",label:"MKV"},{value:"webm",label:"WebM"}].map(item=>({...item,disabled:encoder?!encoder.containers.includes(item.value):false}))}/></div>
 <div className="capture-preview-field"><span>Codec do vídeo</span><SelectionMenu label="Codec do vídeo" onOpen={()=>void loadEncoder()} disabled={disabled} value={settings.codec} onChange={codec=>onChange({codec})} options={[{value:"auto",label:"Automático"},...[{value:"h264",label:"H.264"},{value:"hevc",label:"HEVC (H.265)"},{value:"av1",label:"AV1"},{value:"vp9",label:"VP9"}].map(item=>{const supported=!!encoder?.codecs.includes(item.value),compatible=settings.format!=="webm"||["av1","vp9"].includes(item.value);return {...item,disabled:!supported||!compatible,label:`${item.label}${!supported?" · sem suporte detectado":!compatible?" · use MP4 ou MKV":""}`}})]}/></div>
 </div>
 {encoder&&<p>GPU: {encoder.gpu||encoder.card_path}{encoder.encoder==="NVENC"?" · NVENC":""}. Codecs disponíveis: {encoder.codecs.map(codec=>codec.toUpperCase()).join(", ")}.{!compact&&" Suporte confirmado pelo gravador e driver, sem fallback para CPU."}</p>}
 {!encoder&&!codecError&&<small role="status">Detectando codecs da GPU…</small>}
 {codecError&&<p role="alert" className="capture-error">{codecError}</p>}
 {settings.format==="webm"&&<p>WebM aceita AV1 ou VP9. Escolha um codec disponível na GPU.</p>}
 <div className="capture-video-fields">
  <div className="capture-preview-field"><span>Limite de resolução</span><SelectionMenu label="Limite de resolução" disabled={disabled} value={settings.resolution} onChange={resolution=>onChange({resolution})} options={[{value:"0x0",label:"Original"},{value:"854x480",label:"480p"},{value:"1280x720",label:"720p"},{value:"1920x1080",label:"1080p"},{value:"2560x1440",label:"1440p"},{value:"3840x2160",label:"2160p (4K)"}]}/></div>
  <div className="capture-preview-field"><span>Quadros por segundo</span><SelectionMenu label="FPS da gravação" disabled={disabled} value={String(settings.fps)} onChange={value=>onChange({fps:Number(value)})} options={[24,30,60,90,120,144,165,240].map(value=>({value:String(value),label:`${value} FPS`}))}/></div>
 </div>
 {!compact&&<p>O vídeo mantém a proporção da fonte e é reduzido para caber no limite escolhido. FPS altos exigem mais da GPU; não aumentam o FPS do jogo.</p>}
 <div className="capture-video-fields">
  <div className="capture-preview-field"><span>Controle de qualidade</span><SelectionMenu label="Controle de qualidade" disabled={disabled} value={settings.rate_control} onChange={rate_control=>onChange({rate_control})} options={[{value:"cbr",label:"Bitrate constante (CBR)"},{value:"qp",label:"Qualidade constante (QP)"},{value:"vbr",label:"Bitrate variável (VBR)"}]}/></div>
  {settings.rate_control==="cbr"?<label className="capture-preview-field"><span>Bitrate do vídeo (Mb/s)</span><input className="capture-bitrate-input" type="number" min="1" max="200" step="1" disabled={disabled} value={settings.bitrate_kbps/1000} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>=1&&value<=200)onChange({bitrate_kbps:Math.round(value*1000)});}}/></label>:<div className="capture-preview-field"><span>Qualidade do vídeo</span><SelectionMenu label="Qualidade do vídeo" disabled={disabled} value={settings.quality_preset} onChange={quality_preset=>onChange({quality_preset})} options={[{value:"medium",label:"Média"},{value:"high",label:"Alta"},{value:"very_high",label:"Muito alta"},{value:"ultra",label:"Ultra"}]}/></div>}
 </div>
 <p>{settings.rate_control==="cbr"?"CBR mantém o tamanho do replay mais previsível.":"Qualidade e tamanho dos arquivos variam conforme o conteúdo. O replay também usa esse ajuste."}</p>
 </>}
 {section!=="video"&&<>
 <label className="capture-audio-choice"><input type="checkbox" disabled={disabled} checked={settings.desktop_audio} onChange={e=>onChange({desktop_audio:e.target.checked})}/><span>Capturar áudio{!compact&&<small>Escolha uma saída inteira ou somente um aplicativo.</small>}</span></label>
 {settings.desktop_audio&&<div className="capture-preview-field"><span>Fonte de áudio</span><SelectionMenu label="Fonte de áudio" onOpen={()=>void refresh()} disabled={disabled} value={settings.output_source} onChange={output_source=>onChange({output_source})} options={options([{id:"game:auto",label:"Áudio deste jogo · automático"},...sources.outputs,...sources.applications.map(s=>({...s,label:`Aplicativo · ${s.label}`}))],settings.output_source)}/></div>}
 {settings.desktop_audio&&settings.output_source==="game:auto"&&<p>Detecta o áudio do jogo ao iniciar a captura. O jogo precisa estar aberto e emitindo som. Se houver dúvida, escolha a fonte manualmente.</p>}
 <label className="capture-audio-choice"><input type="checkbox" disabled={disabled} checked={settings.microphone} onChange={e=>onChange({microphone:e.target.checked})}/><span>Microfone{!compact&&<small>Inclui a entrada que você selecionar.</small>}</span></label>
 {settings.microphone&&<div className="capture-preview-field"><span>Entrada do microfone</span><SelectionMenu label="Entrada do microfone" onOpen={()=>void refresh()} disabled={disabled} value={settings.input_source} onChange={input_source=>onChange({input_source})} options={options(sources.inputs,settings.input_source)}/></div>}
 {settings.microphone&&<label className="capture-audio-choice"><input type="checkbox" disabled={disabled} checked={settings.microphone_noise_reduction} onChange={e=>onChange({microphone_noise_reduction:e.target.checked})}/><span>Reduzir ruído do microfone (RNNoise)</span></label>}
 {busy&&<small role="status">Atualizando fontes de áudio…</small>}
 <p>{compact?"Encerre a captura para mudar áudio ou formato.":"Aplicativos aparecem enquanto reproduzem áudio. A captura de uma saída inclui todos os sons enviados a ela, inclusive chamadas. Para mudar áudio ou formato, encerre a captura."}</p>
 {error&&<p role="alert" className="capture-error">{error}</p>}
 </>}
 </>;
}
