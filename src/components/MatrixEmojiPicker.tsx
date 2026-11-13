import {useEffect,useId,useRef,useState} from "react";
import {createPortal} from "react-dom";
import {useBoundedDropdown} from "../hooks/useBoundedDropdown";
import {useI18n} from "../i18n/I18nContext";
import "./matrixEmojiPicker.css";
const EMOJIS=["😀","😃","😄","😁","😆","😂","🤣","😊","🙂","😉","😍","🥰","😘","😎","🤔","🙃","😅","😭","😢","😮","😡","🤯","🥳","😴","👍","👎","👏","🙌","🙏","🤝","👋","💪","❤️","🧡","💛","💚","💙","💜","💔","💯","🔥","✨","🎉","🎮","🕹️","🏆","✅","❌"];
export function MatrixEmojiPicker({disabled,onSelect}:{disabled:boolean;onSelect:(emoji:string)=>void}){
 const {t}=useI18n();const label=t("ui-matrix-chat-panel","insert_emoji");
 const [open,setOpen]=useState(false);const anchor=useRef<HTMLDivElement>(null),menu=useRef<HTMLDivElement>(null),trigger=useRef<HTMLButtonElement>(null);const id=useId();
 const position=useBoundedDropdown(open,anchor,menu,setOpen,260,"right");
 useEffect(()=>{if(open)menu.current?.querySelector<HTMLButtonElement>('button')?.focus()},[open]);
 useEffect(()=>{if(disabled)setOpen(false)},[disabled]);
 const close=()=>{setOpen(false);trigger.current?.focus()};
 return <div ref={anchor} className="matrix-emoji-anchor">
  <button ref={trigger} data-dropdown-trigger type="button" className="matrix-emoji-trigger" aria-label={label} title={label} aria-haspopup="listbox" aria-expanded={open} aria-controls={open?id:undefined} disabled={disabled} onClick={()=>setOpen(v=>!v)}>
   <svg viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M8 14a4.5 4.5 0 0 0 8 0"/><circle cx="9" cy="9" r=".8" fill="currentColor"/><circle cx="15" cy="9" r=".8" fill="currentColor"/></svg>
  </button>
  {open&&!disabled&&createPortal(<div ref={menu} id={id} className="matrix-emoji-picker" role="listbox" aria-label={label} style={{...position,zIndex:2201}}
   onKeyDown={event=>{
    if(event.key==="Escape"){event.preventDefault();event.stopPropagation();close();return}
    if(event.key==="Tab"){event.preventDefault();close();return}
    const step=({ArrowRight:1,ArrowLeft:-1,ArrowDown:8,ArrowUp:-8} as Record<string,number>)[event.key];
    if(!step)return;event.preventDefault();const buttons=[...menu.current?.querySelectorAll<HTMLButtonElement>('button')??[]];const current=buttons.indexOf(document.activeElement as HTMLButtonElement);buttons[(current+step+buttons.length)%buttons.length]?.focus();
   }}>
   {EMOJIS.map(emoji=><button type="button" role="option" aria-selected="false" aria-label={emoji} key={emoji} onClick={()=>{setOpen(false);onSelect(emoji)}}>{emoji}</button>)}
  </div>,document.body)}
 </div>;
}
