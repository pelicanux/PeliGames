import {useMemo, type ReactNode} from "react";
import type {AddonHost} from "../../addons/_infra/ui-sdk";
import {OptiScalerProvider} from "../../addons/optiscaler/ui/src/environment";
import {connectOptiScaler} from "../../addons/optiscaler/ui/src/connection";
import {addonDesign} from "../../src/modules/addons/sharedDesign";
import {AddonGameHost} from "../../src/modules/addons/gameHost";
import {ResourceScope} from "../../src/modules/addons/lifecycle";
import {createAddonI18n} from "../../src/modules/addons/i18n";
import {useI18n} from "../../src/i18n/I18nContext";
import type {GameInfo} from "../../src/components/GameGrid";

/** SSR-only fixture: use real host controls, refuse every native operation. */
export function OptiScalerConnection({children,game}:{children:ReactNode;game?:GameInfo}) {
  const {language}=useI18n();
  const environment=useMemo(()=>{
    const scope=new ResourceScope(()=>{});
    const host:AddonHost={...addonDesign,apiVersion:1,addon:{id:"optiscaler",version:"0.1.0"},language,
      lifecycle:scope,i18n:createAddonI18n(language,scope),styles:{add(){}},setBusy(){},
      service:{async call(){throw new Error("SSR cannot call native services")}},
      files:{async openDirectory(){throw new Error("SSR cannot open files")}},
    };
    return connectOptiScaler(host);
  },[language]);
  return <AddonGameHost game={game??null} onUpdated={()=>{}}><OptiScalerProvider value={environment}>{children}</OptiScalerProvider></AddonGameHost>;
}
