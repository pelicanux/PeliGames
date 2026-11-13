import {useCallback,useEffect,useState,type ReactNode} from "react";
import type {AddonContext} from "../../../addons/_infra/ui-sdk";
import type {ModernTab} from "../../components/modern/ModernGameDetails";
import {ComponentCatalog} from "../../components/ComponentCatalog";
import {ModDetailHeader} from "../../components/ModDetailHeader";
import {useI18n} from "../../i18n/I18nContext";
import {AddonView} from "./AddonView";
import {FeatureReportSection} from "../reports/FeatureReportSection";
import {useAddonReportContext} from "./gameHost";
import {AddonGroupNavigation} from "./gameHost";
import type {AddonOverview,UiContribution} from "./contracts";
import {contributionRoute} from "./contracts";
import {contributionDescription,gameContributions,gameTabTargets,visibleGameTabs,usesBundledTarget} from "./navigation";

type Game=AddonContext["game"];
const key=(item:UiContribution)=>`${item.id}:${item.contribution.route}`;
export function GameAddonDetail({item,game,disabled,onBusyChange,onBack,actions}:{item:UiContribution;game:Game;disabled:boolean;onBusyChange?:(busy:boolean)=>void;onBack:()=>void;actions?:ReactNode}){
 const {t}=useI18n();
 const report=useAddonReportContext(item.group,item.contribution.target,game);
 const content=(reportAction?:ReactNode)=><><ModDetailHeader title={item.name} project={{name:item.name,url:item.repository}} backLabel={t("addons","back")} disabled={disabled} onBack={onBack} actions={<>{actions}{reportAction}</>}/><AddonView route={contributionRoute(item)} game={game} disabled={disabled} onBusyChange={onBusyChange}/></>;
 return report?<FeatureReportSection context={report} label={item.name}>{action=>content(action)}</FeatureReportSection>:content();
}
export function GameAddonPanel({entries,game,disabled,onBusyChange}:{entries:UiContribution[];game:Game;disabled:boolean;onBusyChange?:(busy:boolean)=>void}){
 const {t,language}=useI18n();const [selected,setSelected]=useState<string>(),[group,setGroup]=useState<string>();
 const current=entries.find(item=>key(item)===selected);
 const signature=JSON.stringify(entries.map(item=>[key(item),item.version,item.group,item.contribution.target]));
 useEffect(()=>{if(selected&&!entries.some(item=>key(item)===selected))setSelected(undefined);if(group&&!entries.some(item=>item.group===group))setGroup(undefined)},[signature,selected,group]);
 const openGroup=useCallback((id:string)=>{
  if(!entries.some(item=>item.group===id))throw new Error("Install an add-on from this group first");
  setSelected(undefined);setGroup(id);
 },[signature]);
 if(current)return <AddonGroupNavigation.Provider value={openGroup}><GameAddonDetail item={current} game={game} disabled={disabled} onBusyChange={onBusyChange} onBack={()=>setSelected(undefined)}/></AddonGroupNavigation.Provider>;
 const shown=group?entries.filter(item=>item.group===group):entries.filter(item=>!item.group);
 const groups=group?[]:[...new Set(entries.map(item=>item.group).filter((value):value is string=>Boolean(value)))];
 return <div className="game-modules">
  {group&&<ModDetailHeader title={group==="dlssnr"?"DLSSNR":group} backLabel={t("addons","back")} disabled={disabled} onBack={()=>setGroup(undefined)}/>}
  <ComponentCatalog label={t("addons","installed")} disabled={disabled} cards={[
   ...groups.map(id=>({id:`group:${id}`,label:id==="dlssnr"?"DLSSNR":id,description:t("addons","choose_project"),icon:"graphics" as const})),
   ...shown.map(item=>({id:key(item),label:item.name,description:contributionDescription(item,language),icon:"puzzle" as const})),
  ]} onOpen={id=>{if(id.startsWith("group:"))setGroup(id.slice(6));else setSelected(id)}}/>
 </div>;
}

export function composeAddonGameTabs(tabs:ModernTab[],overview:AddonOverview|undefined,game:Game,disabled:boolean,onBusyChange:(busy:boolean)=>void):ModernTab[]{
 if(!overview)return tabs;
 return visibleGameTabs(tabs,overview).map(tab=>{
  if(tab.id==="overview")return {...tab,content:<>{tab.content}<div className="addon-overview-panels">{gameContributions(overview,"game-overview").map(item=><AddonView key={key(item)} route={contributionRoute(item)} game={game} disabled={disabled}/>)}</div></>};
  const target=gameTabTargets[tab.id];
  if(target&&usesBundledTarget(overview,target))return tab;
  return target?{...tab,disabled:false,content:<GameAddonPanel entries={gameContributions(overview,target)} game={game} disabled={disabled} onBusyChange={onBusyChange}/>,count:undefined}:tab;
 });
}
