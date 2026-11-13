import {featureReportContext,type ReportCategory} from "../reports/reportContext";
import {createContext, useContext, useMemo, useRef, type ReactNode} from "react";
import type {AddonContext, AddonGameActions} from "../../../addons/_infra/ui-sdk";
import type {GameInfo} from "../../components/GameGrid";
import {PeliRunProfile} from "../../components/PeliRunProfile";
import {syncModLaunchOptions} from "../../services/modLaunchOptions";
import {appConfigStore} from "../appConfig/appConfig";
import {invoke} from "@tauri-apps/api/core";
import {updatePeliGamesSettings} from "../../services/installedLibrary";
import {normalizeAdvanced,gamescopeEnabled,defaultGamescope,isFrameGenerationWrapper} from "../../services/advancedSettings";
import {addonStore} from "./store";
import {addonLaunchData,applyAddonLaunchSettings} from "./launchSettings";

interface GameBinding {resolvedExecutable?:string;running?:boolean;persistAdvanced?:(settings:import("../../services/advancedSettings").AdvancedSettings)=>Promise<void>; game: GameInfo | null; onUpdated: (game: GameInfo) => void; onChooseExecutableDirectory?: () => Promise<void>}
const Binding = createContext<GameBinding | undefined>(undefined);
export const AddonGroupNavigation = createContext<((group: string) => void) | undefined>(undefined);
export function AddonGameHost({children, ...binding}: GameBinding & {children: ReactNode}) {
  return <Binding.Provider value={binding}>{children}</Binding.Provider>;
}
export function AddonLaunchProfile({gameId, ...props}: {gameId: string; disabled?: boolean; inline?: boolean; actionLabel?: string}) {
  const binding = useContext(Binding);
  return binding?.game?.path === gameId ? <PeliRunProfile {...props} game={binding.game}/> : null;
}
/** Core facilities are scoped to the selected game; add-ons never receive library records. */
export function useAddonGameActions(addonId: string, game: AddonContext["game"]): AddonGameActions | undefined {
  const binding = useContext(Binding);
  const navigate = useContext(AddonGroupNavigation);
  const latest = useRef(binding);
  latest.current = binding;
  return useMemo(() => {
    if (!game || !binding?.game || binding.game.path !== game.id) return undefined;
    const selected = (gameId: string) => {
      const current = latest.current;
      if (gameId !== game.id || current?.game?.path !== gameId) throw new Error("The selected game changed");
      return current as GameBinding & {game: GameInfo};
    };
    return {
      launch: {
        read: async gameId => {
          const current=selected(gameId),game=current.game,settings=normalizeAdvanced(game.advanced);
          const projections=settings.wrappers?.length?await invoke<NonNullable<typeof settings.addon_launch>>("addon_launch_settings",{advanced:settings}):settings.addon_launch;
          const own=projections?.[addonId],data=own?{...own.data,enabled:own.enabled}:addonLaunchData(game,addonId);
          const executable=await invoke<{bits:number|null;windows:boolean}>("addon_game_probe",{executable:current.resolvedExecutable||game.executable||game.path});selected(gameId);
          return {executablePath:current.resolvedExecutable||game.executable||"",steamAppId:game.launcher==="Steam"?game.app_id||"":"",gameKey:game.external_id||game.path,gameName:game.name,directory:game.directory||game.path,wrappers:settings.wrappers?.map(({program,enabled})=>({program,enabled})),settings:data,flatpak:Boolean(game.flatpak),gamescope:gamescopeEnabled(settings.gamescope??defaultGamescope),running:Boolean(current.running||game.nexus_running||game.nexus_starting),supported:Boolean(current.persistAdvanced)||game.launcher==="PeliGames"||Boolean(game.external_id),options:Object.keys(settings.options).filter(key=>settings.options[key]),competing:Boolean(settings.wrappers?.some(wrapper=>wrapper.enabled&&isFrameGenerationWrapper(wrapper))||Object.entries(settings.addon_launch??{}).some(([id,value])=>id!==addonId&&value.enabled&&value.group==="frame-generation")),executable};
        },
        save: async (gameId,data) => {
          const current=selected(gameId),game=current.game;
          if(current.running||game.nexus_running||game.nexus_starting)throw new Error("Close the game before changing launch settings");
          const overview=addonStore.getSnapshot().overview;
          const installed=overview?.state.addons[addonId],manifest=installed?.versions[installed.selected??""]?.manifest;
          if(!overview||!installed?.enabled||!manifest)throw new Error("The integration is unavailable");
          const advanced=applyAddonLaunchSettings(game,addonId,manifest.category??null,data,overview);
          const updated=current.persistAdvanced?await current.persistAdvanced(advanced).then(()=>({...game,advanced})):game.external_id?await invoke<GameInfo>("configure_external_game",{settings:{external_id:game.external_id,advanced}}):await updatePeliGamesSettings({path:game.path,name:game.name,executable:game.executable||game.path,proton:game.proton||"",prefix:game.prefix,advanced});
          const latest=selected(gameId);latest.onUpdated({...game,...updated,library_view:game.library_view});window.dispatchEvent(new Event("peligamesLibraryChanged"));
        },
      },
      synchronizeLaunch: async (gameId, directory, line) => {
        const current = selected(gameId);
        const updated = await syncModLaunchOptions(current.game, directory, line);
        if (updated && latest.current?.game?.path === gameId) latest.current.onUpdated(updated);
        return Boolean(updated);
      },
      saveMenuShortcut: async (gameId, key) => {
        selected(gameId);
        await appConfigStore.update({game_shortcut_keys: {[`${gameId}::menu:${addonId}`]: key}});
      },
      refresh: async gameId => {selected(gameId);await invoke("addon_game_refresh",{directory:game.executableDirectory||game.directory});selected(gameId);window.dispatchEvent(new Event("gameInfoCacheCleared"));window.dispatchEvent(new Event("peligamesLibraryChanged"));},
      chooseExecutableDirectory: async gameId => {
        const current=selected(gameId);
        if(!current.onChooseExecutableDirectory)throw new Error("Game directory selection is unavailable");
        await current.onChooseExecutableDirectory();
      },
      openGroup: group => {
        selected(game.id);
        if (!navigate) throw new Error("Add-on group navigation is unavailable");
        navigate(group);
      },
    };
  }, [addonId, game?.id, Boolean(binding?.game?.path === game?.id), navigate]);
}

/** Reports remain a host facility; integrations receive no account tokens. */
export function useAddonReportContext(group:string|null|undefined,target:string,game:AddonContext["game"]){
 const binding=useContext(Binding);
 if(!game||binding?.game?.path!==game.id)return undefined;
 const category:ReportCategory|undefined=group==="dlssnr"?"dlssnr":target==="game-frame-generation"?"frame-generation":target==="game-captures"?"captures":undefined;
 return category?featureReportContext(binding.game,category,game.directory,game.executableDirectory):undefined;
}
