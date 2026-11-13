import {useId,type ComponentProps,type ReactNode} from "react";
import {MenuIcon} from "./MenuIcon";
import "./settingsCategories.css";

export interface SettingsCategory<T extends string> {
 id:T;label:string;icon:ComponentProps<typeof MenuIcon>["name"];disabled?:boolean;disabledHint?:string;content:ReactNode;
}
/** Shared vertical navigation; inactive pages stay mounted to preserve pending edits. */
export function SettingsCategories<T extends string>({label,active,onChange,categories,className=""}:{label:string;active:T;onChange:(id:T)=>void;categories:readonly SettingsCategory<T>[];className?:string}){
 const id=useId();
 return <div className={`settings-categories ${className}`}>
  <div className="settings-categories-tabs" role="tablist" aria-orientation="vertical" aria-label={label} onKeyDown={event=>{
   if(!["ArrowUp","ArrowDown","Home","End"].includes(event.key))return;
   const available=categories.filter(item=>!item.disabled);if(!available.length)return;
   event.preventDefault();
   const index=available.findIndex(item=>item.id===active);
   const next=available[event.key==="Home"?0:event.key==="End"?available.length-1:(index+(event.key==="ArrowDown"?1:available.length-1))%available.length];
   onChange(next.id);document.getElementById(`${id}-${next.id}`)?.focus();
  }}>
   {categories.map(item=><button key={item.id} type="button" role="tab" id={`${id}-${item.id}`} aria-selected={active===item.id} aria-controls={`${id}-${item.id}-panel`} tabIndex={active===item.id?0:-1} disabled={item.disabled} title={item.disabled?item.disabledHint:item.label} onClick={()=>onChange(item.id)}><MenuIcon name={item.icon}/><span>{item.label}</span></button>)}
  </div>
  {categories.map(item=><section className="settings-category-content" role="tabpanel" tabIndex={0} id={`${id}-${item.id}-panel`} hidden={active!==item.id} aria-labelledby={`${id}-${item.id}`} key={item.id}>{item.content}</section>)}
 </div>;
}
