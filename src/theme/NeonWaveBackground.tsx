import {openrgbDisplayColor} from "./openrgbColor";
import {NeonWaveAudioResponse} from "./neonWaveAudioResponse";
import {useOpenRGB} from "./openrgbSync";
import {useEffect,useRef} from "react";
import {createPortal} from "react-dom";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {useNeonWaveOptions,setNeonWaveAudioState} from "./neonWavePreferences";
type Bands={session:string;bass:number;mid:number;treble:number;error?:string};
const colors=[[169,60,247],[244,61,150],[30,185,211],[106,79,243],[225,91,48],[59,117,219]];
let audioQueue=Promise.resolve();
function audio(enabled:boolean,session:string){audioQueue=audioQueue.catch(()=>{}).then(()=>invoke<void>("set_neon_wave_audio",{enabled,session}));return audioQueue;}
export function NeonWaveBackground({active}:{active:boolean}){
 const canvasRef=useRef<HTMLCanvasElement>(null),options=useNeonWaveOptions();
 const sync=useOpenRGB();const synced=useRef(sync.color);synced.current=sync.preferences.enabled&&sync.color?openrgbDisplayColor(sync.color):null;
 const intensity=useRef(options.intensity);intensity.current=options.intensity;
 const reaction=useRef(options.reaction);reaction.current=options.reaction;
 useEffect(()=>{
  if(!active)return;
  const canvas=canvasRef.current,ctx=canvas?.getContext("2d",{alpha:false});if(!canvas||!ctx)return;
  const preview="__PELI_UI_PREVIEW__" in window,motion=matchMedia("(prefers-reduced-motion: reduce)");
  const session=crypto.randomUUID();let disposed=false,timer:ReturnType<typeof setInterval>|undefined,reading=false,paused=true,last=performance.now(),clock=0,received=0;
  let target=[0,0,0],bands=[0,0,0];
  const response=new NeonWaveAudioResponse();
  const draw=()=>{
   const now=performance.now(),dt=Math.min(.1,(now-last)/1000);last=now;if(!paused)clock+=dt;
   if(preview&&options.audio&&options.demo&&!paused){const beat=Math.pow(Math.max(0,Math.sin(clock*Math.PI*3.6)),9);target=[beat*.85,(Math.sin(clock*4.1)+1)*.18,(Math.sin(clock*8.4)+1)*.12];}
   else if(now-received>350)target=[0,0,0];
   bands=bands.map((n,i)=>{const level=target[i]*reaction.current/100;return n+(level-n)*(1-Math.exp(-dt*(level>n?24:7)));});
   const w=canvas.width,h=canvas.height,strength=intensity.current/100;
   ctx.globalCompositeOperation="source-over";ctx.fillStyle="#080918";ctx.fillRect(0,0,w,h);
   ctx.globalCompositeOperation="screen";
   const palette=synced.current?colors.map((base,i)=>base.map((n,c)=>Math.round(synced.current![c]*(i%2?.72:.9)+n*(i%2?.28:.1)))):colors;
   palette.forEach(([r,g,b],i)=>{
    const phase=i*1.67,t=clock*(.105+i*.009),pulse=bands[i%3],beat=bands[0];
    const x=w*(.5+Math.sin(t+phase)*.4+Math.sin(t*.7+phase)*.1+Math.sin(phase)*pulse*.045),y=h*(.5+Math.cos(t*.83+phase)*.32+Math.cos(phase)*pulse*.045);
    const radius=Math.max(w,h)*(.29+Math.sin(t*.6+phase)*.04+pulse*.24+beat*.055);
    ctx.save();ctx.translate(x,y);ctx.rotate(Math.sin(t*.4+phase));ctx.scale(1,.75+Math.sin(t+phase)*.15);
    const gradient=ctx.createRadialGradient(0,0,0,0,0,radius);
    const a=Math.min(.95,strength*(.28+pulse*.95+beat*.18));gradient.addColorStop(0,`rgba(${r},${g},${b},${a})`);gradient.addColorStop(.38,`rgba(${r},${g},${b},${a*.7})`);gradient.addColorStop(1,`rgba(${r},${g},${b},0)`);
    ctx.fillStyle=gradient;ctx.beginPath();ctx.arc(0,0,radius,0,Math.PI*2);ctx.fill();ctx.restore();
   });
   ctx.globalCompositeOperation="source-over";
  };
  const resize=()=>{canvas.width=Math.max(1,Math.min(480,innerWidth));canvas.height=Math.max(1,Math.min(360,Math.round(canvas.width*innerHeight/Math.max(innerWidth,1))));draw();};
  const listeners=preview||!isTauri()?Promise.resolve([]):Promise.all([
   listen<Bands>("neon-wave-bands",({payload})=>{if(disposed||payload.session!==session||paused)return;received=performance.now();target=response.update([payload.bass,payload.mid,payload.treble],received);setNeonWaveAudioState("listening");}),
   listen<{session:string;error:string}>("neon-wave-audio-error",({payload})=>{if(!disposed&&payload.session===session)setNeonWaveAudioState("unavailable",payload.error);})
  ]);
  const capture=async(want:boolean)=>{
   if(reading===want)return;reading=want;
   if(!preview&&isTauri()){
    try{await listeners;if(disposed&&want)return;await audio(want,session);}catch(error){if(!disposed)setNeonWaveAudioState("unavailable",String(error));}
   }
  };
  const pause=()=>{
   clearInterval(timer);paused=document.hidden||motion.matches||document.body.classList.contains("performance-mode");
   setNeonWaveAudioState(paused?"paused":!options.audio?"off":preview?(options.demo?"demo":"off"):"connecting");
   void capture(!paused&&options.audio);last=performance.now();
   if(!paused)timer=setInterval(draw,1000/30);else{target=[0,0,0];bands=[0,0,0];draw();}
  };
  const observer=new MutationObserver(pause);observer.observe(document.body,{attributes:true,attributeFilter:["class"]});
  resize();pause();window.addEventListener("resize",resize);document.addEventListener("visibilitychange",pause);motion.addEventListener("change",pause);
  return()=>{disposed=true;clearInterval(timer);observer.disconnect();window.removeEventListener("resize",resize);document.removeEventListener("visibilitychange",pause);motion.removeEventListener("change",pause);void listeners.then(items=>items.forEach(stop=>stop())).catch(()=>{});if(!preview&&isTauri())void audio(false,session).catch(()=>{});setNeonWaveAudioState("off");};
 },[active,options.audio,options.demo]);
 return active?createPortal(<canvas ref={canvasRef} className="neon-wave-background" aria-hidden="true"/>,document.body):null;
}
