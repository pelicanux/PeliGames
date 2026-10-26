import {useEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";
import {useBoundedDropdown} from "../../hooks/useBoundedDropdown";
export function ReplayDurationField({seconds,disabled,onChange,compact=false}:{seconds:number;disabled:boolean;compact?:boolean;onChange:(seconds:number)=>void}){
 const presets=[60,180,300];
 const [open,setOpen]=useState(false),[custom,setCustom]=useState(!presets.includes(seconds)),[draft,setDraft]=useState(String(seconds));
 const anchor=useRef<HTMLDivElement>(null),menu=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null),input=useRef<HTMLInputElement>(null);
 const position=useBoundedDropdown(open,anchor,menu,setOpen,205,"right");
 const valid=draft.trim()!==""&&Number.isInteger(Number(draft))&&Number(draft)>=1&&Number(draft)<=900;
 useEffect(()=>{if(disabled)setOpen(false);},[disabled]);
 useEffect(()=>{if(custom&&!open){input.current?.focus({preventScroll:true});input.current?.select();}},[custom]);
 const commit=()=>{if(valid)onChange(Number(draft));else setDraft(String(seconds));};
 const toggle=()=>setOpen(value=>!value);
 const close=()=>{setOpen(false);trigger.current?.focus({preventScroll:true});};
 const label=presets.includes(seconds)?`${seconds/60} ${seconds===60?"minuto":"minutos"}`:seconds%60===0?`${seconds/60} minutos`:`${seconds} segundos`;
 return <div className={`capture-preview-field capture-replay-duration${compact?" compact":""}`}>
 {!compact&&<span>Duração do replay</span>}
 <div ref={anchor} className="custom-select-wrapper selection-menu">
 {custom?<div className={`custom-select-trigger capture-duration-editor${open?" open":""}`}>
 <input ref={input} type="number" aria-label="Tempo do replay em segundos" min={1} max={900} step={1} value={draft} disabled={disabled} aria-invalid={!valid} onChange={event=>setDraft(event.target.value)} onBlur={commit} onKeyDown={event=>{if(event.key==="Enter"){event.preventDefault();commit();input.current?.blur();}if(event.key==="Escape"){event.stopPropagation();setDraft(String(seconds));input.current?.blur();}}}/><span className="capture-duration-unit">s</span><button ref={trigger} type="button" aria-label="Duração do replay" aria-haspopup="dialog" aria-expanded={open} disabled={disabled} onClick={toggle}><span className="select-arrow" aria-hidden="true">⌄</span></button></div>
 :<button ref={trigger} type="button" className={`custom-select-trigger${open?" open":""}`} aria-label="Duração do replay" aria-haspopup="dialog" aria-expanded={open} disabled={disabled} onClick={toggle}><span>{label}</span><span className="select-arrow" aria-hidden="true">⌄</span></button>}
 {open&&!disabled&&createPortal(<div ref={menu} role="dialog" aria-label="Duração do replay" className="custom-select-menu selection-options capture-duration-menu" style={{...position,zIndex:2201}} onKeyDown={event=>{if(event.key==="Escape"){event.preventDefault();event.stopPropagation();close();}}}>
 <div className="selection-options-scroll">
 {presets.map(value=><button type="button" key={value} className={`custom-select-option${seconds===value&&!custom?" selected":""}`} onClick={()=>{setCustom(false);setDraft(String(value));onChange(value);close();}}><span>{value/60} {value===60?"minuto":"minutos"}</span>{seconds===value&&!custom&&<span aria-hidden="true">✓</span>}</button>)}
 <button type="button" className={`custom-select-option${custom?" selected":""}`} onClick={()=>{setDraft(String(seconds));setOpen(false);setCustom(true);requestAnimationFrame(()=>{input.current?.focus({preventScroll:true});input.current?.select();});}}>Personalizado</button>
 </div></div>,document.body)}
 </div></div>;
}
