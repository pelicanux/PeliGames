import {useLayoutEffect,useRef,useState} from "react";
import {createPortal} from "react-dom";

/** Shared surface and viewport positioning for explicit and native-title hints. */
export function TooltipBubble({text,anchor,id,align="right",className=""}:{text:string;anchor:HTMLElement;id:string;align?:"left"|"right";className?:string}){
 const tooltip=useRef<HTMLDivElement>(null);
 const [position,setPosition]=useState<{left:number;top:number;zIndex:number}>();
 useLayoutEffect(()=>{
  const place=()=>{
   if(!anchor.isConnected||!tooltip.current)return;
   const bounds=anchor.getBoundingClientRect(),hint=tooltip.current.getBoundingClientRect(),margin=8;
   const minTop=(document.querySelector(".app-titlebar")?.getBoundingClientRect().bottom??0)+margin;
   const above=bounds.top-hint.height-margin;
   let zIndex=2101;
   for(let parent:HTMLElement|null=anchor;parent;parent=parent.parentElement){const layer=Number.parseInt(getComputedStyle(parent).zIndex,10);if(Number.isFinite(layer))zIndex=Math.max(zIndex,layer+1);}
   setPosition({zIndex,left:Math.max(margin,Math.min(align==="left"?bounds.left:bounds.right-hint.width,window.innerWidth-hint.width-margin)),top:Math.max(minTop,Math.min(above>=minTop?above:bounds.bottom+margin,window.innerHeight-hint.height-margin))});
  };
  place();const observer=new ResizeObserver(place);if(tooltip.current)observer.observe(tooltip.current);observer.observe(anchor);
  window.addEventListener("resize",place);window.addEventListener("scroll",place,true);
  return()=>{observer.disconnect();window.removeEventListener("resize",place);window.removeEventListener("scroll",place,true);};
 },[anchor,text,align]);
 return createPortal(<div ref={tooltip} id={id} role="tooltip" className={`ui-hover-tooltip ${className}`} style={{zIndex:position?.zIndex,left:position?.left??0,top:position?.top??0,visibility:position?"visible":"hidden"}}>{text}</div>,document.body);
}
