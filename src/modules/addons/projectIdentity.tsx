import {useState} from "react";
import {MenuIcon} from "../../components/MenuIcon";
import type {AddonManifest} from "./contracts";
import {manifestCategory} from "./navigation";

// Curated upstream marks are local assets; unknown packages keep the host's icon system.
const marks:Readonly<Record<string,string>>={afmf:"/addons/afmf.svg",mako:"/addons/mako.png","gpu-screen-recorder":"/addons/gpu-screen-recorder.png"};
export function projectReference(manifest:AddonManifest){
 return {name:manifest.name,url:manifest.id==="optiscaler"?"https://github.com/optiscaler/OptiScaler":manifest.repository};
}
export function AddonProjectIcon({manifest}:{manifest:AddonManifest}){
 const [failed,setFailed]=useState(false),source=marks[manifest.id],category=manifestCategory(manifest);
 return <span className="addon-project-icon" data-project={manifest.id}>
  {source&&!failed?<img src={source} alt="" onError={()=>setFailed(true)}/>:<MenuIcon name={category==="frame-generation"?"bolt":category==="captures"?"platform":manifest.group==="dlssnr"?"neural":manifest.id==="optiscaler"?"graphics":"puzzle"}/>}
 </span>;
}
