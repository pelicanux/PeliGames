import {useLayoutEffect,useRef,useState,type ReactNode} from "react";

/** Reuse real preference controls and their handlers. Only marked matches and
 * their labels/context remain visible; async content is filtered on insertion. */
export function PreferenceSearchScope({ids,children}:{ids:readonly string[];children:ReactNode}){
 const root=useRef<HTMLDivElement>(null);
 const [found,setFound]=useState(true);
 const signature=ids.join("|");
 useLayoutEffect(()=>{
  const node=root.current;if(!node)return;
  const matches=new Set(ids);let frame=0;
  const filter=()=>{
   node.querySelectorAll("[data-search-hidden]").forEach(el=>el.removeAttribute("data-search-hidden"));
   const visit=(el:Element):boolean=>{
    const id=el.getAttribute("data-preference-id");
    if(id&&matches.has(id))return true;
    const branches=Array.from(el.children).map(child=>[child,visit(child)] as const);
    const matched=branches.some(([,yes])=>yes);
    if(matched){for(const [child,yes] of branches){const context=child.matches("h2,h3,h4,[role=alert],[role=status]")&&!child.querySelector("[data-preference-id]");if(!yes&&!context)child.setAttribute("data-search-hidden","");}}
    return matched;
   };
   let any=false;for(const child of Array.from(node.children)){const matched=visit(child);any=any||matched;if(!matched&&!child.matches("[role=alert],[role=status]"))child.setAttribute("data-search-hidden","");}
   setFound(old=>old===any?old:any);
  };
  filter();const observer=new MutationObserver(()=>{cancelAnimationFrame(frame);frame=requestAnimationFrame(filter)});observer.observe(node,{childList:true,subtree:true});
  return()=>{observer.disconnect();cancelAnimationFrame(frame)};
 },[signature]);
 return <><div className="preference-search-scope" ref={root}>{children}</div>{!found&&<p role="status" className="preference-search-unavailable">Esta opção não está disponível nesta instalação ou ainda está sendo carregada.</p>}</>;
}
