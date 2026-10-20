import {useEffect,useId,useRef,useState,type ReactNode} from "react";
import {useI18n} from "../../i18n/I18nContext";
import "./ModernLibraryFilters.css";

/** Source filters stay accessible without a horizontally scrolling toolbar. */
export function ModernLibraryFilters({children,activeLabel}:{children:ReactNode;activeLabel?:string}){
 const {language}=useI18n(),pt=language==="pt",id=useId();
 const [opened,setOpened]=useState(false);const anchor=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null),panel=useRef<HTMLDivElement>(null);
 useEffect(()=>{if(!opened)return;panel.current?.querySelector<HTMLButtonElement>('button[aria-pressed="true"],button')?.focus();const dismiss=(event:PointerEvent)=>{if(event.target instanceof Node&&!anchor.current?.contains(event.target))setOpened(false);};const escape=(event:KeyboardEvent)=>{if(event.key==="Escape"){event.stopPropagation();setOpened(false);trigger.current?.focus();}};document.addEventListener("pointerdown",dismiss);document.addEventListener("keydown",escape);return()=>{document.removeEventListener("pointerdown",dismiss);document.removeEventListener("keydown",escape);};},[opened]);
 return <div ref={anchor} className="modern-library-filter" onBlur={event=>{if(event.relatedTarget instanceof Node&&!event.currentTarget.contains(event.relatedTarget))setOpened(false);}}>
  <button ref={trigger} type="button" className="modern-lib-icon modern-lib-labeled modern-library-filter-trigger" aria-expanded={opened} aria-controls={id} aria-haspopup="dialog" onClick={()=>setOpened(value=>!value)}>
   <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true"><path d="M4 5h16l-6 7v6l-4 2v-8z"/></svg><span>{pt?"Filtrar":"Filter"}{activeLabel?` · ${activeLabel}`:""}</span>
  </button>
  {opened&&<div ref={panel} id={id} className="modern-library-filter-popover" role="dialog" aria-label={pt?"Filtrar biblioteca":"Filter library"} onClick={event=>{if((event.target as Element).closest("button")){setOpened(false);trigger.current?.focus();}}} onKeyDown={event=>{if(!["ArrowDown","ArrowUp","Home","End"].includes(event.key))return;const buttons=Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>("button"));const index=buttons.indexOf(document.activeElement as HTMLButtonElement);const next=event.key==="Home"?0:event.key==="End"?buttons.length-1:(index+(event.key==="ArrowUp"?-1:1)+buttons.length)%buttons.length;event.preventDefault();buttons[next]?.focus();}}>{children}</div>}
 </div>;
}
