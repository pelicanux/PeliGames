import {useLayoutEffect,useRef,useState,type CSSProperties} from "react";

/** Pan a single clipped subtitle on hover, fading back to its start between passes. */
export function NavSubtitle({text}:{text:string}){
 const viewport=useRef<HTMLElement>(null),content=useRef<HTMLSpanElement>(null);
 const [distance,setDistance]=useState(0);
 useLayoutEffect(()=>{
  const measure=()=>{if(viewport.current&&content.current)setDistance(Math.max(0,Math.ceil(content.current.getBoundingClientRect().width)-viewport.current.clientWidth));};
  measure();const observer=new ResizeObserver(measure);
  if(viewport.current)observer.observe(viewport.current);if(content.current)observer.observe(content.current);
  return()=>observer.disconnect();
 },[text]);
 return <small ref={viewport} className="modern-nav-subtitle" data-overflow={distance>1?"true":undefined} style={{"--subtitle-travel":`${distance}px`, "--subtitle-duration":`${Math.max(5,distance/24+2.5)}s`} as CSSProperties}><span ref={content}>{text}</span></small>;
}
