import {useEffect, useRef, useState, type ReactNode} from "react";
import type {AddonContext} from "../../../addons/_infra/ui-sdk";
import {contributionDescription} from "../addons/navigation";
import type {UiContribution} from "../addons/contracts";
import {AddonGroupNavigation} from "../addons/gameHost";
import {GameAddonDetail} from "../addons/GameAddonPanels";
import {useI18n} from "../../i18n/I18nContext";
import {ComponentCatalog, type ComponentCard} from "../../components/ComponentCatalog";
import {ModDetailHeader} from "../../components/ModDetailHeader";
import {addonProjects} from "../../config/addonProjects";
import "./gameModsTabs.css";

export type GameModsTab = "catalog" | "nexus" | "dlss";
interface Props {
  activeTab: GameModsTab; onTabChange: (tab: GameModsTab) => void;
  nexus: ReactNode; disabled?: boolean;
  externalEntries?: UiContribution[]; game?: AddonContext["game"]; onBusyChange?: (busy: boolean) => void;
}
/** Nexus remains integrated; mod backends are discovered from enabled add-ons. */
export function GameModsTabs({activeTab, onTabChange, nexus, disabled=false, externalEntries=[], game, onBusyChange}: Props) {
  const {t,language} = useI18n();
  const text = (key: "mods_for_this_game" | "choose_type" | "nexus_hint" | "nr_hint" | "choose_backend" | "all_mods" | "backends") => t("ui-game-mods-tabs",key);
  const panel=useRef<HTMLDivElement>(null), previous=useRef(activeTab);
  const [selectedExternal,setSelectedExternal]=useState<string>();
  const current=externalEntries.find(item=>`${item.id}:${item.contribution.route}`===selectedExternal);
  useEffect(()=>{if(selectedExternal&&!current)setSelectedExternal(undefined);},[selectedExternal,current]);
  useEffect(()=>{if(previous.current!==activeTab){setSelectedExternal(undefined);panel.current?.focus({preventScroll:true});previous.current=activeTab;}},[activeTab]);
  const nr=externalEntries.filter(item=>item.group==="dlssnr");
  const roots:ComponentCard<string>[]=[
    ...(nr.length?[{id:"dlss",label:"DLSSNR",description:text("nr_hint"),icon:"neural" as const}]:[]),
    ...externalEntries.filter(item=>item.group!=="dlssnr").map(item=>({id:`addon:${item.id}:${item.contribution.route}`,label:item.name,description:contributionDescription(item,language),icon:"puzzle" as const})),
    {id:"nexus",label:"Nexus Mods",description:text("nexus_hint"),icon:"puzzle"},
  ];
  const backends:ComponentCard<string>[]=nr.map(item=>({id:`addon:${item.id}:${item.contribution.route}`,label:item.name,description:contributionDescription(item,language),icon:"graphics"}));
  const open=(id:string)=>{if(id.startsWith("addon:"))setSelectedExternal(id.slice(6));else onTabChange(id as GameModsTab)};
  if(current)return <AddonGroupNavigation.Provider value={group=>{
    if(group!=="dlssnr")throw new Error("Install an add-on from this group first");
    setSelectedExternal(undefined);onTabChange("dlss");
  }}><GameAddonDetail item={current} game={game} disabled={disabled} onBusyChange={onBusyChange} onBack={()=>setSelectedExternal(undefined)}/></AddonGroupNavigation.Provider>;

  return <div className="game-modules">
    {activeTab!=="catalog"&&<ModDetailHeader title={activeTab==="dlss"?"DLSSNR":"Nexus Mods"} project={activeTab==="nexus"?addonProjects.nexus[0]:undefined} backLabel={text("all_mods")} disabled={disabled} onBack={()=>onTabChange("catalog")}/>}
    <div ref={panel} tabIndex={-1} className="game-modules-content" aria-label={text("mods_for_this_game")}>
      {activeTab==="catalog" ? <><p className="modern-dlss-description">{text("choose_type")}</p><ComponentCatalog label={text("mods_for_this_game")} cards={roots} disabled={disabled} onOpen={open}/></>
        : activeTab==="dlss" ? <><p className="modern-dlss-description">{text("choose_backend")}</p><ComponentCatalog label={text("backends")} cards={backends} disabled={disabled} onOpen={open}/></>
        : nexus}
    </div>
  </div>;
}
