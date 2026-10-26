import {useEffect,useId,useState} from "react";
import {TooltipBubble} from "./TooltipBubble";

/** Adapt legacy title hints without wrapping controls or observing the whole UI. */
export function NativeTitleTooltips(){
 const id=useId();
 const [visible,setVisible]=useState<{anchor:HTMLElement;text:string}>();
 useEffect(()=>{
  let current:{element:HTMLElement;text:string;dismissed:boolean;custom:boolean;temporaryLabel?:string}|undefined;
  let timer:ReturnType<typeof setTimeout>|undefined;
  let observer:MutationObserver|undefined;
  const clearDescription=(element:HTMLElement)=>{const ids=(element.getAttribute("aria-describedby")||"").split(/\s+/).filter(value=>value&&value!==id);if(ids.length)element.setAttribute("aria-describedby",ids.join(" "));else element.removeAttribute("aria-describedby");};
  const dismiss=()=>{if(timer)clearTimeout(timer);timer=undefined;if(current){current.dismissed=true;clearDescription(current.element);}setVisible(undefined);};
  const close=()=>{observer?.disconnect();observer=undefined;const previous=current;dismiss();current=undefined;if(previous){if(previous.element.getAttribute("title")==="")previous.element.setAttribute("title",previous.text);if(previous.temporaryLabel&&previous.element.getAttribute("aria-label")===previous.temporaryLabel)previous.element.removeAttribute("aria-label");}};
  const show=()=>{if(!current||current.dismissed||current.custom||!current.element.isConnected)return;const ids=new Set((current.element.getAttribute("aria-describedby")||"").split(/\s+/).filter(Boolean));ids.add(id);current.element.setAttribute("aria-describedby",[...ids].join(" "));setVisible({anchor:current.element,text:current.text});};
  const activate=(target:EventTarget|null,immediate=false)=>{
   if(!(target instanceof Element))return;
   const element=target.closest<HTMLElement>("[title]");
   if(!element||!element.getAttribute("title")?.trim()){if(current?.element===element)return;close();return;}
   if(current?.element===element)return;
   close();
   current={element,text:element.getAttribute("title")!,dismissed:false,custom:Boolean(element.closest(".hover-tooltip-anchor"))};
   if(element.matches("button,a,[role=button]")&&!element.hasAttribute("aria-label")&&!element.hasAttribute("aria-labelledby")&&!element.textContent?.trim()){current.temporaryLabel=current.text;element.setAttribute("aria-label",current.text);}
   element.setAttribute("title","");
   observer=new MutationObserver(()=>{
    if(!current||current.element!==element)return;
    const text=element.getAttribute("title");
    if(!text?.trim()){close();if(text===null)element.removeAttribute("title");else element.setAttribute("title",text);return;}
    if(current.temporaryLabel&&element.getAttribute("aria-label")===current.temporaryLabel){current.temporaryLabel=text;element.setAttribute("aria-label",text);}
    current.text=text;
    observer?.disconnect();element.setAttribute("title","");observer?.observe(element,{attributes:true,attributeFilter:["title"]});
    if(!timer)show();
   });
   observer.observe(element,{attributes:true,attributeFilter:["title"]});
   if(immediate)show();else timer=setTimeout(()=>{timer=undefined;show();},300);
  };
  const enter=(event:PointerEvent)=>{if(event.pointerType!=="touch")activate(event.target);};
  const leave=(event:PointerEvent)=>{if(current&&(!(event.relatedTarget instanceof Node)||!current.element.contains(event.relatedTarget)))close();};
  const focus=(event:FocusEvent)=>{if(event.target instanceof Element&&event.target.matches(":focus-visible"))activate(event.target,true);};
  const blur=(event:FocusEvent)=>{if(current&&(!(event.relatedTarget instanceof Node)||!current.element.contains(event.relatedTarget)))close();};
  const escape=(event:KeyboardEvent)=>{if(event.key==="Escape")dismiss();};
  document.addEventListener("pointerover",enter,true);document.addEventListener("pointerout",leave,true);
  document.addEventListener("focusin",focus,true);document.addEventListener("focusout",blur,true);
  document.addEventListener("pointerdown",dismiss,true);document.addEventListener("keydown",escape,true);
  window.addEventListener("blur",close);window.addEventListener("scroll",close,true);
  return()=>{close();document.removeEventListener("pointerover",enter,true);document.removeEventListener("pointerout",leave,true);document.removeEventListener("focusin",focus,true);document.removeEventListener("focusout",blur,true);document.removeEventListener("pointerdown",dismiss,true);document.removeEventListener("keydown",escape,true);window.removeEventListener("blur",close);window.removeEventListener("scroll",close,true);};
 },[id]);
 return visible?<TooltipBubble id={id} text={visible.text} anchor={visible.anchor} className="native-title-tooltip"/>:null;
}
