import {useLayoutEffect,useRef,type CSSProperties} from "react";
import "./matrixPlayingTicker.css";

/** One compositor animation; duplicates are decorative, the label is read once. */
export function MatrixPlayingTicker({text,className=""}:{text:string;className?:string}){
 const viewport=useRef<HTMLSpanElement>(null),copy=useRef<HTMLSpanElement>(null);
 useLayoutEffect(()=>{
  const element=viewport.current,label=copy.current;if(!element||!label)return;
  const measure=()=>{const distance=label.getBoundingClientRect().width+28;element.style.setProperty("--playing-distance",`${-distance}px`);element.style.setProperty("--playing-duration",`${Math.max(8,distance/24)}s`);};
  const resize=new ResizeObserver(measure);resize.observe(label);measure();
  const visible=new IntersectionObserver(entries=>{element.dataset.paused=String(!entries[0]?.isIntersecting||document.hidden);});visible.observe(element);
  const visibility=()=>{element.dataset.paused=String(document.hidden||!element.getClientRects().length);};
  document.addEventListener("visibilitychange",visibility);
  return()=>{resize.disconnect();visible.disconnect();document.removeEventListener("visibilitychange",visibility);};
 },[text]);
 return <span ref={viewport} className={`matrix-playing-ticker ${className}`} title={text} aria-label={text} style={{"--playing-distance":"-180px","--playing-duration":"10s"} as CSSProperties}>
  <span className="matrix-playing-track" aria-hidden="true"><span ref={copy}>{text}</span><span>{text}</span></span>
 </span>;
}
