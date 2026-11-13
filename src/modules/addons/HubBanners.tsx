import {useEffect,useRef,useState} from "react";
import {MatrixIcon} from "../../components/MatrixIcon";
import {MenuIcon} from "../../components/MenuIcon";
import {ProjectLink} from "../../components/ProjectLink";
import {useI18n} from "../../i18n/I18nContext";
import {SUPPORT_PROJECT_URL,YOUTUBE_MEMBERSHIP_URL} from "../../services/communityLinks";
import {OPEN_MATRIX_CHAT_EVENT} from "../../services/matrixChat";
import "./hubBanners.css";

const banners=[
 {id:"discord",url:"https://discord.gg/78XSB9bHst",icon:"discord" as const},
 {id:"matrix",icon:"language" as const},
 {id:"support",url:SUPPORT_PROJECT_URL,icon:"heart" as const},
 {id:"members",url:YOUTUBE_MEMBERSHIP_URL,icon:"bolt" as const},
] as const;
/** Auto-rotation pauses during interaction, while hidden and outside the viewport. */
export function HubBanners({disabled=false}: {disabled?:boolean}){
 const {t}=useI18n(),[selected,setSelected]=useState(0);
 const frame=useRef<HTMLElement>(null);
 const [hovered,setHovered]=useState(false),[focused,setFocused]=useState(false),[paused,setPaused]=useState(false),[visible,setVisible]=useState(true),[inView,setInView]=useState(true);
 useEffect(()=>{
  const visibility=()=>setVisible(!document.hidden);visibility();document.addEventListener("visibilitychange",visibility);
  const observer=typeof IntersectionObserver!=="undefined"?new IntersectionObserver(entries=>setInView(entries.some(entry=>entry.isIntersecting))):undefined;
  if(frame.current)observer?.observe(frame.current);
  return()=>{document.removeEventListener("visibilitychange",visibility);observer?.disconnect()};
 },[]);
 useEffect(()=>{
  if(paused||hovered||focused||!visible||!inView)return;
  const timer=window.setTimeout(()=>setSelected(index=>(index+1)%banners.length),7000);
  return()=>window.clearTimeout(timer);
 },[selected,paused,hovered,focused,visible,inView]);
 const move=(direction:number)=>setSelected(index=>(index+direction+banners.length)%banners.length);
 const banner=banners[selected];
 return <section ref={frame} className="addon-hub-banner" data-banner={banner.id} aria-label={t("addons","community_banner")}
  onMouseEnter={()=>setHovered(true)} onMouseLeave={()=>setHovered(false)}
  onFocusCapture={()=>setFocused(true)} onBlurCapture={event=>{if(!event.currentTarget.contains(event.relatedTarget as Node|null))setFocused(false)}}>
  <img key={banner.id} className="addon-hub-banner-art" src={`/addons/banners/${banner.id}.webp`} alt=""/>
  <img className="addon-hub-banner-logo" src="/branding/peligames-glass.webp" alt=""/>
  <div className="addon-hub-banner-copy"><span className="addon-hub-banner-label">{banner.id==="matrix"?<MatrixIcon/>:<MenuIcon name={banner.icon}/>}{t("addons",`banner_${banner.id}_label`)}</span>
   <h3>{t("addons",`banner_${banner.id}_title`)}</h3><p>{t("addons",`banner_${banner.id}_description`)}</p>
   {banner.id==="matrix"?<button type="button" className="addon-hub-banner-action" disabled={disabled} onClick={()=>window.dispatchEvent(new Event(OPEN_MATRIX_CHAT_EVENT))}>{t("addons","banner_matrix_action")}</button>:<ProjectLink project={{name:t("addons",`banner_${banner.id}_label`),url:banner.url}}>{t("addons",`banner_${banner.id}_action`)}</ProjectLink>}
   {banner.id==="members"&&<ProjectLink project={{name:"Discord",url:banners[0].url}}>{t("addons","banner_members_boost")}</ProjectLink>}
  </div>
  <div className="addon-hub-banner-pages" role="group" aria-label={t("addons","banner_select")}>{banners.map((item,index)=><button type="button" key={item.id} aria-label={t("addons",`banner_${item.id}_label`)} aria-pressed={index===selected} onClick={()=>setSelected(index)}><span/></button>)}</div>
  <div className="addon-hub-banner-controls">
   <button type="button" className="addon-hub-banner-arrow previous" aria-label={t("addons","banner_previous")} onClick={()=>move(-1)}><MenuIcon name="chevronDown"/></button>
   <button type="button" className="addon-hub-banner-pause" aria-label={t("addons",paused?"banner_play":"banner_pause")} onClick={()=>setPaused(!paused)}>{paused?<MenuIcon name="play"/>:<svg width="14" height="14" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true"><rect x="5" y="4" width="3" height="12" rx="1"/><rect x="12" y="4" width="3" height="12" rx="1"/></svg>}</button>
   <button type="button" className="addon-hub-banner-arrow next" aria-label={t("addons","banner_next")} onClick={()=>move(1)}><MenuIcon name="chevronDown"/></button>
  </div>
 </section>;
}
