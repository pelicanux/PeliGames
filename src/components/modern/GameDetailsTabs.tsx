import {useLayoutEffect, useRef, useState, type KeyboardEvent} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {HoverTooltip} from "../HoverTooltip";
import type {ModernTab} from "./ModernGameDetails";

/** All game tabs remain reachable when their labels exceed the window width. */
export function GameDetailsTabs({tabs, activeId, onChange}: {tabs: ModernTab[]; activeId?: string; onChange: (id: string) => void}) {
  const pt=useI18n().language === "pt";
  const viewport=useRef<HTMLDivElement>(null);
  const [edges,setEdges]=useState({overflow:false,left:false,right:false});
  const signature=JSON.stringify(tabs.map(tab=>[tab.id,tab.label,tab.count,tab.disabled]));
  const reveal=(button:HTMLElement)=>{
    const row=viewport.current;if(!row)return;
    const box=button.getBoundingClientRect(), frame=row.getBoundingClientRect();
    const delta=box.left<frame.left ? box.left-frame.left-4 : box.right>frame.right ? box.right-frame.right+4 : 0;
    if(delta)row.scrollLeft+=delta;
  };
  useLayoutEffect(()=>{
    const row=viewport.current;if(!row)return;
    const measure=()=>{
      const overflow=row.scrollWidth>row.clientWidth+1;
      const next={overflow,left:overflow&&row.scrollLeft>1,right:overflow&&row.scrollLeft+row.clientWidth<row.scrollWidth-1};
      setEdges(old=>old.overflow===next.overflow&&old.left===next.left&&old.right===next.right?old:next);
    };
    const resize=()=>{const selected=row.querySelector<HTMLElement>('[aria-selected="true"]');if(selected)reveal(selected);measure();};
    const wheel=(event:WheelEvent)=>{
      if(event.ctrlKey||event.shiftKey||Math.abs(event.deltaX)>=Math.abs(event.deltaY))return;
      const delta=event.deltaY*(event.deltaMode===1?16:event.deltaMode===2?row.clientWidth:1);
      if(row.scrollWidth<=row.clientWidth+1 || (delta<0&&row.scrollLeft<=0) || (delta>0&&row.scrollLeft+row.clientWidth>=row.scrollWidth-1))return;
      event.preventDefault();row.scrollLeft+=delta;
    };
    const observer=new ResizeObserver(resize);observer.observe(row);
    row.querySelectorAll('[role="tab"]').forEach(button=>observer.observe(button));
    row.addEventListener('scroll',measure,{passive:true});row.addEventListener('wheel',wheel,{passive:false});resize();
    return()=>{observer.disconnect();row.removeEventListener('scroll',measure);row.removeEventListener('wheel',wheel);};
  },[signature]);
  useLayoutEffect(()=>{const selected=viewport.current?.querySelector<HTMLElement>('[aria-selected="true"]');if(selected)reveal(selected);},[activeId,signature]);
  const move=(direction:number)=>{
    const row=viewport.current;if(row)row.scrollBy({left:direction*Math.max(180,row.clientWidth*.65),behavior:window.matchMedia('(prefers-reduced-motion: reduce)').matches?'auto':'smooth'});
  };
  const keyboard=(event:KeyboardEvent<HTMLDivElement>)=>{
    if(!['ArrowLeft','ArrowRight','Home','End'].includes(event.key))return;
    const buttons=Array.from(event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="tab"]:not(:disabled)'));
    const index=buttons.indexOf(event.target as HTMLButtonElement);if(index<0)return;
    event.preventDefault();
    const next=buttons[event.key==='Home'?0:event.key==='End'?buttons.length-1:(index+(event.key==='ArrowRight'?1:-1)+buttons.length)%buttons.length];
    next?.focus({preventScroll:true});next?.click();if(next)reveal(next);
  };
  return <div className="modern-game-tabs" data-overflow={edges.overflow || undefined}>
    <button type="button" className="modern-tab-scroll" aria-label={pt?"Mostrar abas anteriores":"Show previous tabs"} disabled={!edges.left} hidden={!edges.overflow} onClick={()=>move(-1)}><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m15 5-7 7 7 7"/></svg></button>
    <div ref={viewport} className="modern-tabs" role="tablist" aria-label={pt?"Configurações do jogo":"Game settings"} onKeyDown={keyboard}>
      {tabs.map(item=>item.disabled?<HoverTooltip key={item.id} text={item.disabledHint || (pt?"Em desenvolvimento":"In development")}><button type="button" role="tab" disabled aria-selected={false} tabIndex={-1}>{item.label}</button></HoverTooltip>:<button type="button" key={item.id} role="tab" aria-selected={activeId===item.id} tabIndex={activeId===item.id?0:-1} onClick={()=>onChange(item.id)}>{item.label}{item.count?<span className="modern-tab-count">{item.count}</span>:null}</button>)}
    </div>
    <button type="button" className="modern-tab-scroll" aria-label={pt?"Mostrar próximas abas":"Show next tabs"} disabled={!edges.right} hidden={!edges.overflow} onClick={()=>move(1)}><svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m9 5 7 7-7 7"/></svg></button>
  </div>;
}
