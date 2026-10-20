import {useRef,useState} from "react";
import type {MangoHudSettings} from "../../services/mangohud";
import {movableMangoItem,orderedMangoGroups,orderedMangoItems,placeMangoGroup,placeMangoItem} from "./mangoHudGroups";
type Drag={kind:"group"|"item";id:string;group:string};
export function MangoHudMetrics({settings,onToggle,onOrder}:{settings:MangoHudSettings;onToggle:(key:string,value:boolean|string)=>void;onOrder:(order:string)=>void}){
 const source=useRef<Drag|null>(null),suppressClick=useRef(false);
 const [target,setTarget]=useState("");
 const finish=()=>{source.current=null;setTarget("");};
 const begin=(event:React.DragEvent,value:Drag)=>{event.stopPropagation();source.current=value;suppressClick.current=true;event.dataTransfer.effectAllowed="move";event.dataTransfer.setData("text/plain",value.id);};
 const groups=orderedMangoGroups(settings);
 return <div className="mm-groups">{groups.map((group,index)=><div key={group.id} className={`mm-group ${target===group.id?"mm-group-drop":""}`}
   onDragOver={event=>{if(source.current?.kind==="group"){event.preventDefault();setTarget(group.id);}}}
   onDrop={event=>{if(source.current?.kind!=="group")return;event.preventDefault();onOrder(placeMangoGroup(settings,source.current.id,group.id));finish();}}>
   <h3 draggable onDragStart={event=>begin(event,{kind:"group",id:group.id,group:group.id})} onDragEnd={finish} title="Arraste para mover a categoria inteira">
    <i style={{background:group.color}}/><span>{group.title}</span><span className="mm-group-grip" aria-hidden="true">⠿</span>
    <button type="button" aria-label={`Subir ${group.title}`} disabled={!index} onClick={()=>onOrder(placeMangoGroup(settings,group.id,groups[index-1].id))}>↑</button>
    <button type="button" aria-label={`Descer ${group.title}`} disabled={index===groups.length-1} onClick={()=>onOrder(placeMangoGroup(settings,group.id,groups[index+1].id))}>↓</button>
   </h3>
   <div className="mm-chips">{orderedMangoItems(settings,group.items).filter(([key])=>key in settings).map(([key,label,on])=>{
    const active=key==="fps_metrics"?String(settings[key]||"").split(",").includes("0.01"):on?settings[key]===on:Boolean(settings[key]);
    const movable=movableMangoItem(key);
    return <button type="button" key={key} draggable={movable} className={`mm-chip ${target===key?"mm-chip-drop":""}`} aria-pressed={active}
      title={movable?"Clique para ligar/desligar. Arraste para mover o bloco dentro da categoria; Alt + setas também reorganiza.":"Clique para ligar/desligar. A posição deste sensor acompanha a linha principal no MangoHUD."}
      onDragStart={event=>begin(event,{kind:"item",id:key,group:group.id})} onDragEnd={finish}
      onDragOver={event=>{if(source.current?.kind==="item"&&source.current.group===group.id){event.preventDefault();event.stopPropagation();setTarget(key);}}}
      onDrop={event=>{event.preventDefault();event.stopPropagation();if(source.current?.kind==="item"&&source.current.group===group.id)onOrder(placeMangoItem(settings,source.current.id,key));finish();}}
      onPointerDown={()=>{suppressClick.current=false;}}
      onKeyDown={event=>{if(!event.altKey||!movable||!["ArrowLeft","ArrowRight","ArrowUp","ArrowDown"].includes(event.key))return;event.preventDefault();const items=orderedMangoItems(settings,group.items).filter(([id])=>movableMangoItem(id));const delta=["ArrowLeft","ArrowUp"].includes(event.key)?-1:1;const to=items[items.findIndex(([id])=>id===key)+delta];if(to)onOrder(placeMangoItem(settings,key,to[0]));}}
      onClick={()=>{if(suppressClick.current){suppressClick.current=false;return;}onToggle(key,on?(active?"":on):!settings[key]);}}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="3"><path d="m5 12 5 5 9-10"/></svg>{label}
    </button>;
   })}</div>
 </div>)}</div>;
}
