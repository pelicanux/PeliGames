import { invoke, isTauri } from "@tauri-apps/api/core";
import "./matrixRtcProbe.css";
const get = <T extends HTMLElement>(id:string) => document.getElementById(id)! as T;
const media = navigator.mediaDevices;
const native = isTauri();
const senders = typeof RTCRtpSender !== "undefined" && typeof RTCRtpSender.getCapabilities === "function";
const codecs = (kind:"audio"|"video") => {
  try { return senders ? RTCRtpSender.getCapabilities(kind)?.codecs.map(c=>c.mimeType) ?? [] : []; } catch { return []; }
};
const capabilities = {
  ambiente:native ? "Aplicativo Tauri" : "Navegador · não valida o aplicativo instalado",
  contextoSeguro:window.isSecureContext,
  WebRTC:typeof RTCPeerConnection === "function",
  microfoneCamera:typeof media?.getUserMedia === "function",
  capturaTela:typeof media?.getDisplayMedia === "function",
  codecsAudio:codecs("audio"),
  codecsVideo:codecs("video"),
};
// Debug banner: helps diagnose if WebRTC is unavailable in the native webview.
{
  const banner = document.createElement("p");
  const rtcOk = typeof RTCPeerConnection === "function";
  banner.style.cssText = `margin:8px 0;padding:8px 12px;border-radius:6px;font-size:13px;font-family:monospace;background:${rtcOk?"#1a3a1a":"#3a1a1a"};color:${rtcOk?"#6fdd6f":"#dd6f6f"};border:1px solid ${rtcOk?"#3a6a3a":"#6a3a3a"}`;
  banner.textContent = `RTCPeerConnection: ${rtcOk ? "✓ disponível" : "✗ indisponível nesta webview"} · ${native ? "Tauri nativo" : "Navegador"} · contexto seguro: ${window.isSecureContext}`;
  document.querySelector("main")?.prepend(banner);
}
get("capabilities").textContent = JSON.stringify(capabilities,null,2);
if(native) void invoke("matrix_rtc_probe_report",{report:capabilities}).catch(()=>{});
if (native) void invoke<{platform:string;session:string;desktop:string;rtc_transports:string[];discovery_error:string|null}>("matrix_rtc_probe_environment").then(result=>{
  get("environment").textContent = `Sistema: ${result.platform} · Sessão: ${result.session || "não identificada"} · Desktop: ${result.desktop || "não identificado"}`;
  get("transport").textContent = result.discovery_error ? `Descoberta Matrix: ${result.discovery_error}` : result.rtc_transports.length ? `Transporte anunciado: ${result.rtc_transports.join(", ")}. Acesso à chamada ainda não testado.` : "Nenhum transporte MatrixRTC anunciado.";
}).catch(()=>{get("environment").textContent="Não foi possível ler o diagnóstico nativo.";});
else get("environment").textContent = "Teste de navegador. Microfone, câmera e tela só são solicitados ao clicar no teste correspondente.";

let stream:MediaStream|null=null, audio:AudioContext|null=null, tick=0, generation=0, pending=false;
let peers:RTCPeerConnection[]=[], rtcBusy=false, alive=true;
const status = (text:string) => get("capture-status").textContent=text;
const updateButtons = () => {
  get<HTMLButtonElement>("microphone").disabled = pending || !media?.getUserMedia;
  get<HTMLButtonElement>("camera").disabled = pending || !media?.getUserMedia;
  get<HTMLButtonElement>("screen").disabled = pending || !media?.getDisplayMedia;
  get<HTMLButtonElement>("stop").disabled = !pending && !stream;
};
function stopCapture() {
  ++generation; cancelAnimationFrame(tick);
  stream?.getTracks().forEach(track=>track.stop()); stream=null;
  if (audio) void audio.close().catch(()=>{}); audio=null;
  get<HTMLVideoElement>("preview").srcObject=null; get<HTMLVideoElement>("preview").hidden=true; get<HTMLMeterElement>("level").value=0;
  status(pending ? "Cancelado. Aguardando o encerramento da solicitação do sistema…" : "Captura encerrada."); updateButtons();
}
const mediaError = (error:unknown) => {
  const name = error instanceof DOMException ? error.name : "Error";
  return ({NotAllowedError:"Permissão negada ou cancelada.",NotFoundError:"Nenhum dispositivo disponível.",NotReadableError:"Dispositivo ocupado ou captura indisponível.",NotSupportedError:"Captura não suportada por este ambiente.",OverconstrainedError:"O dispositivo não atende ao teste solicitado."} as Record<string,string>)[name] ?? `Falha na captura (${name}).`;
};
async function capture(kind:"microphone"|"camera"|"screen") {
  if (pending) return;
  stopCapture(); const run=generation; pending=true; updateButtons(); status("Aguardando autorização do sistema…");
  try {
    const next=kind==="screen" ? await media.getDisplayMedia({video:true,audio:false}) : await media.getUserMedia({audio:kind==="microphone",video:kind==="camera"});
    if (!alive || run!==generation) {next.getTracks().forEach(t=>t.stop());return;}
    stream=next; next.getTracks().forEach(track=>track.addEventListener("ended",()=>{if(stream===next)stopCapture();},{once:true}));
    if(kind==="microphone") {
      audio=new AudioContext(); await audio.resume();
      if (!alive || run!==generation) return;
      const source=audio.createMediaStreamSource(next), analyser=audio.createAnalyser(); analyser.fftSize=256; source.connect(analyser);
      const samples=new Uint8Array(analyser.fftSize);
      const measure=()=>{if(stream!==next)return;analyser.getByteTimeDomainData(samples);const rms=Math.sqrt(samples.reduce((sum,s)=>sum+((s-128)/128)**2,0)/samples.length);get<HTMLMeterElement>("level").value=Math.min(100,rms*350);tick=requestAnimationFrame(measure);};measure();
    }else {get<HTMLVideoElement>("preview").hidden=false;get<HTMLVideoElement>("preview").srcObject=next;await get<HTMLVideoElement>("preview").play();}
    if(alive && run===generation)status(`${kind==="screen"?"Tela/janela":kind==="camera"?"Câmera":"Microfone"} funcionando na prévia local. Sem transmissão ou gravação.`);
  }catch(error){if(alive && run===generation){stopCapture();status(mediaError(error));}}
  finally{pending=false;if(alive)updateButtons();}
}
get("microphone").onclick=()=>void capture("microphone");get("camera").onclick=()=>void capture("camera");get("screen").onclick=()=>void capture("screen");get("stop").onclick=stopCapture;updateButtons();

get<HTMLButtonElement>("loopback").disabled=typeof RTCPeerConnection!=="function";
if(typeof RTCPeerConnection!=="function" && native) {
  get("rtc-status").textContent = "WebRTC indisponível: RTCPeerConnection não existe nesta webview. Verifique o log do terminal para MATRIX_RTC_NATIVE.";
}
get("loopback").onclick=async()=>{
  if(rtcBusy)return;rtcBusy=true;get<HTMLButtonElement>("loopback").disabled=true;get("rtc-status").textContent="Testando conexão local…";
  let timeout:ReturnType<typeof setTimeout>|undefined;
  try {
    const a=new RTCPeerConnection({iceServers:[]});peers=[a];const b=new RTCPeerConnection({iceServers:[]});peers.push(b);
    const queues=[[] as RTCIceCandidate[],[] as RTCIceCandidate[]];
    const route=(source:RTCPeerConnection,target:RTCPeerConnection,index:number)=>source.onicecandidate=event=>{if(!event.candidate)return;if(target.remoteDescription)void target.addIceCandidate(event.candidate).catch(()=>{});else queues[index].push(event.candidate);};
    route(a,b,0);route(b,a,1);
    const channel=a.createDataChannel("local-probe");
    const result=new Promise<void>((resolve,reject)=>{
      timeout=setTimeout(()=>reject(new Error("O transporte local não conectou em 10 segundos.")),10000);
      b.ondatachannel=event=>{event.channel.onmessage=message=>{if(message.data==="peligames-local-test")event.channel.send("ok");};};
      channel.onopen=()=>channel.send("peligames-local-test");channel.onmessage=event=>{if(event.data==="ok")resolve();};channel.onerror=()=>reject(new Error("Falha no canal WebRTC local."));
    });
    // Attach a rejection handler while the offer/answer exchange is pending.
    void result.catch(()=>{});
    const negotiate=async()=>{
    await a.setLocalDescription(await a.createOffer());await b.setRemoteDescription(a.localDescription!);
    for(const ice of queues[0])await b.addIceCandidate(ice);queues[0]=[];
    await b.setLocalDescription(await b.createAnswer());await a.setRemoteDescription(b.localDescription!);
    for(const ice of queues[1])await a.addIceCandidate(ice);queues[1]=[];
    };
    await Promise.race([negotiate().then(()=>result),result]);if(alive)get("rtc-status").textContent="Conexão WebRTC local concluída. Ainda falta validar mídia e chamada Matrix entre duas contas/redes.";
  }catch(error){if(alive)get("rtc-status").textContent=error instanceof Error?error.message:"Falha no teste WebRTC.";}
  finally{clearTimeout(timeout);peers.forEach(peer=>peer.close());peers=[];rtcBusy=false;if(alive)get<HTMLButtonElement>("loopback").disabled=false;}
};
window.addEventListener("pagehide",()=>{alive=false;stopCapture();peers.forEach(peer=>peer.close());},{once:true});

let callActive = false;
// Independent native path: never falls back silently to browser microphone capture.
async function setupNativeVoice() {
  if (!native) return;
  const { listen } = await import("@tauri-apps/api/event");
  const start = get<HTMLButtonElement>("native-start"), stop = get<HTMLButtonElement>("native-stop");
  const mode = get<HTMLSelectElement>("native-noise");
  const nativeStatus = get("native-status");
  let active = false;
  const reset = () => {active=false;start.disabled=callActive;stop.disabled=true;mode.disabled=callActive;get<HTMLMeterElement>("native-input").value=0;get<HTMLMeterElement>("native-output").value=0;};
  const unmeter = await listen<{input:number;output:number}>("matrix-voice-meter",event=>{
    if(!active)return;
    get<HTMLMeterElement>("native-input").value=event.payload.input;
    get<HTMLMeterElement>("native-output").value=event.payload.output;
  });
  const unend = await listen<string|null>("matrix-voice-ended",event=>{
    reset();nativeStatus.textContent=event.payload ?? "Teste nativo encerrado. Nenhum áudio foi transmitido ou gravado.";
  });
  reset();nativeStatus.textContent="Pronto. Clique para autorizar o teste local do microfone por até 30 segundos.";
  start.onclick=async()=>{
    if(active)return;active=true;start.disabled=true;mode.disabled=true;
    nativeStatus.textContent="Preparando o microfone e o filtro nativo…";
    try {await invoke("matrix_voice_start",{mode:mode.value});if(active){stop.disabled=false;nativeStatus.textContent="Microfone nativo ativo. Comparando o nível antes e depois do filtro; encerramento automático em 30 segundos.";}}
    catch(error){reset();nativeStatus.textContent=String(error);}
  };
  stop.onclick=async()=>{stop.disabled=true;try{await invoke("matrix_voice_stop");reset();}catch(error){stop.disabled=false;nativeStatus.textContent=String(error);}};
  window.addEventListener("pagehide",()=>{unmeter();unend();void invoke("matrix_voice_stop").catch(()=>{});},{once:true});
}
void setupNativeVoice().catch(()=>{get("native-status").textContent="Não foi possível preparar o teste nativo. Reabra esta janela no aplicativo atualizado.";});

async function setupCall() {
  if(!native)return;
  const {listen}=await import("@tauri-apps/api/event");
  const start=get<HTMLButtonElement>("call-start"),stop=get<HTMLButtonElement>("call-stop"),mute=get<HTMLButtonElement>("call-mute"),volume=get<HTMLInputElement>("call-volume"),message=get("call-status");
  let active=false,muted=true;
  const reset=()=>{active=false;callActive=false;muted=true;start.disabled=false;stop.disabled=true;mute.disabled=true;volume.disabled=true;mute.textContent="Ativar microfone";get<HTMLButtonElement>("native-start").disabled=false;get<HTMLSelectElement>("native-noise").disabled=false;};
  const statusOff=await listen<{state:string;participants:number}>("matrix-call-status",e=>{if(!active)return;mute.disabled=false;volume.disabled=false;message.textContent=`Conectado · ${e.payload.participants} participantes. Microfone ${muted?"silenciado":"ativo"}.`;});
  const endOff=await listen<string|null>("matrix-call-ended",e=>{reset();message.textContent=e.payload??"Chamada encerrada.";});
  start.disabled=false;
  start.onclick=async()=>{if(active)return;active=true;callActive=true;muted=true;start.disabled=true;stop.disabled=false;volume.value="50";get<HTMLButtonElement>("native-start").disabled=true;get<HTMLSelectElement>("native-noise").disabled=true;message.textContent="Conectando ao servidor local…";try{await invoke("matrix_call_start",{mode:get<HTMLSelectElement>("native-noise").value});}catch(e){reset();message.textContent=String(e);}};
  stop.onclick=async()=>{stop.disabled=true;message.textContent="Encerrando chamada…";try{await invoke("matrix_call_stop");reset();message.textContent="Chamada encerrada.";}catch(e){message.textContent=String(e);stop.disabled=false;}};
  const controls=async(next:boolean)=>{try{await invoke("matrix_call_controls",{muted:next,volume:Number(volume.value)});muted=next;mute.textContent=muted?"Ativar microfone":"Silenciar microfone";message.textContent=muted?"Microfone silenciado. Você continua ouvindo a outra sessão.":"Microfone ativo: transmitindo para a outra sessão.";}catch(e){message.textContent=String(e);}};
  mute.onclick=()=>void controls(!muted);volume.oninput=()=>void controls(muted);
  window.addEventListener("pagehide",()=>{statusOff();endOff();void invoke("matrix_call_stop").catch(()=>{});},{once:true});
}
void setupCall().catch(()=>{get("call-status").textContent="Não foi possível preparar o teste de chamada.";});
