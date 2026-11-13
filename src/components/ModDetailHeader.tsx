import type {ReactNode} from "react";
import {useI18n} from "../i18n/I18nContext";
import type {ProjectReference} from "../config/addonProjects";
import {ProjectLink} from "./ProjectLink";
import {MenuIcon} from "./MenuIcon";
import "./modDetailHeader.css";

export function ModDetailHeader({title,project,backLabel,onBack,disabled,actions}:{title:string;project?:ProjectReference;backLabel?:string;onBack?:()=>void;disabled?:boolean;actions?:ReactNode}){
 const {t}=useI18n();
 return <header className="mod-detail-header">
  {onBack&&<button type="button" className="modern-mini-btn" disabled={disabled} onClick={onBack}><MenuIcon name="back"/>{backLabel}</button>}
  <div className="mod-detail-heading"><h3>{title}</h3>{project&&<ProjectLink project={project}>{t("addonCatalogData","visit_project")}</ProjectLink>}</div>
  {actions&&<div className="mod-detail-actions">{actions}</div>}
 </header>;
}
