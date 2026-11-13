import type {ReactNode} from "react";
import type {ProjectReference} from "../config/addonProjects";
import {openBrowserUrl} from "../services/browser";
import {notifyError} from "../services/notifications";
import {MenuIcon} from "./MenuIcon";

export function ProjectLink({project,children}:{project:ProjectReference;children?:ReactNode}){
 return <a href={project.url} target="_blank" rel="noopener noreferrer" title={project.name} onClick={event=>{
  if("__PELI_UI_PREVIEW__" in window)return;
  event.preventDefault();void openBrowserUrl(project.url).catch(error=>notifyError(project.name,String(error)));
 }}>{children??project.name}<MenuIcon name="externalLink"/></a>;
}
