import {useI18n} from "../../i18n/I18nContext";
import {addonProjects} from "../../config/addonProjects";
import {ProjectLink} from "../ProjectLink";
import type {AddonId} from "./addonCatalogData";

export function AddonProjectLinks({addon,projects}:{addon?:AddonId;projects?:readonly {name:string;url:string}[]}) {
  const {t}=useI18n();
  const label=t("addonCatalogData","support_projects");
  return <footer className="addon-project-links" aria-label={label}>
    <span>{label}</span>
    {(projects??(addon?addonProjects[addon]:[])).map(project=><ProjectLink key={project.url} project={project}/>)}
  </footer>;
}
