import type {OptiScalerMessageKey} from "./messages";
import {useState} from "react";
import type {AddonHost, AddonInstance, AddonViewProps} from "../../../_infra/ui-sdk";
import {OptiScalerProvider} from "./environment";
import {OptiScalerPreferences} from "./OptiScalerPreferences";
import {UpscalerPanel} from "./UpscalerPanel";
import projects from "../projects.json";
import {connectOptiScaler} from "./connection";

export function createAddon(host: AddonHost): AddonInstance {
  const environment = connectOptiScaler(host);
  const translate = (key: OptiScalerMessageKey) => environment.t("upscaler", key);
  function Preferences({disabled}: AddonViewProps) {
    return <OptiScalerProvider value={environment}><OptiScalerPreferences disabled={disabled} onBusy={host.setBusy}/><host.ui.Projects projects={projects}/></OptiScalerProvider>;
  }
  function Game({context, disabled}: AddonViewProps) {
    const [choosing,setChoosing]=useState(false),[error,setError]=useState("");
    if (!context.game || context.game.native) return <p>{translate("native_unavailable")}</p>;
    const game = context.game;
    if(!game.executableDirectory)return <div>
      <p>{translate("choose_destination_hint")}</p>
      <host.ui.Button disabled={disabled||choosing||!host.game} onClick={()=>{
        if(!host.game||choosing)return;setChoosing(true);setError("");host.setBusy(true);
        void host.game.chooseExecutableDirectory(game.id).catch(error=>{if(!host.lifecycle.signal.aborted)setError(String(error))})
          .finally(()=>{if(!host.lifecycle.signal.aborted){setChoosing(false);host.setBusy(false)}});
      }}>{translate("choose_destination")}</host.ui.Button>
      {error&&<p role="alert">{error}</p>}
    </div>;
    return <OptiScalerProvider value={environment}><UpscalerPanel directory={game.executableDirectory} game={{path: game.id}} blocked={disabled}
      onBusy={host.setBusy} onRefresh={async () => {await host.game?.refresh(game.id)}}
      onShortcut={async key => {await host.game?.saveMenuShortcut(game.id, key)}}
      onManageNr={() => host.game?.openGroup("dlssnr")}/></OptiScalerProvider>;
  }
  return {apiVersion: 1, id: host.addon.id, version: host.addon.version, views: {preferences: Preferences, game: Game}, dispose() {}};
}
