import {useCallback,useLayoutEffect,type RefObject} from "react";

/** Grow with wrapped text, then keep a scrollable viewport of at most maxRows. */
export function useAutosizeTextarea(ref:RefObject<HTMLTextAreaElement|null>,value:string,visible:boolean,maxRows:number){
 const resize=useCallback(()=>{
  const element=ref.current;
  if(!element)return;
  element.style.overflowY="hidden";
  element.style.height="auto";
  const style=getComputedStyle(element);
  const lineHeight=Number.parseFloat(style.lineHeight);
  const padding=Number.parseFloat(style.paddingTop)+Number.parseFloat(style.paddingBottom);
  const border=Number.parseFloat(style.borderTopWidth)+Number.parseFloat(style.borderBottomWidth);
  const maximum=lineHeight*maxRows+padding+border;
  const minimum=lineHeight+padding+border;
  const contentHeight=element.scrollHeight+border;
  element.style.height=`${Math.min(maximum,Math.max(minimum,contentHeight))}px`;
  element.style.overflowY=contentHeight>Math.ceil(maximum)?"auto":"hidden";
 },[ref,maxRows]);
 useLayoutEffect(()=>{if(visible)resize()},[resize,value,visible]);
 useLayoutEffect(()=>{
  const element=ref.current;
  if(!visible||!element)return;
  let width=element.clientWidth;
  const observer=new ResizeObserver(()=>{
   if(element.clientWidth===width)return;
   width=element.clientWidth;resize();
  });
  observer.observe(element);
  let active=true;
  void document.fonts.ready.then(()=>{if(active)resize()});
  return()=>{active=false;observer.disconnect()};
 },[ref,visible,resize]);
}
