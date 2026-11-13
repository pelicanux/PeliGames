import {useEffect, useRef} from "react";
import {experimentalDanielNr} from "../../services/releaseFeatures";
import {OPEN_DANIEL_PREFERENCES} from "../neuralRendering/danielNr";
import {OPEN_FRAME_GENERATION_PREFERENCES} from "../../services/frameGenerationNavigation";
import type {SettingsPage} from "../../components/SettingsModal";
export type PreferencesSection = "general" | "covers" | "protons" | "backendUpdate" | "monitoring" | "diskAutomount" | "interface" | "connections" | "nexusAccount" | "appimageInstall" | "mako" | "capture" | "mods" | "daniel" | `addon:${string}`;
interface Requests {
  openAddonStore: () => void;
  coverBlocked: boolean;
  runnerBlocked: boolean;
  featureBlocked: boolean;
  open: (section: PreferencesSection, page?: SettingsPage, closeMenu?: boolean) => void;
}
/** Event bridges read the latest guards without installing a second navigation state. */
export function usePreferencesRequests(requests: Requests) {
  const current = useRef(requests); current.current = requests;
  useEffect(() => {
    const store = () => current.current.openAddonStore();
    const covers = () => {if (!current.current.coverBlocked) current.current.open("covers");};
    const runners = () => {if (!current.current.runnerBlocked) current.current.open("protons", "protons", true);};
    const monitoring = () => {if (!current.current.featureBlocked) current.current.open("monitoring", "monitoring");};
    const frame = (event: Event) => {
      const backend: unknown = (event as CustomEvent).detail;
      if ((backend === "mako" || backend === "capture") && !current.current.featureBlocked) current.current.open(backend, "mods", true);
    };
    const daniel = () => {if (experimentalDanielNr && !current.current.featureBlocked) current.current.open("daniel", "mods", true);};
    const addon=(event:Event)=>{const id=(event as CustomEvent<unknown>).detail;if(typeof id==="string"&&/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/.test(id)&&!current.current.featureBlocked)current.current.open(`addon:${id}`,"mods",true);};
    const bindings = [["openAddonStore",store],["openAddonPreferences",addon],["openCoverPreferences", covers], ["openProtonPreferences", runners], ["openMonitoringPreferences", monitoring], [OPEN_FRAME_GENERATION_PREFERENCES, frame], [OPEN_DANIEL_PREFERENCES, daniel]] as const;
    for (const [name, handler] of bindings) window.addEventListener(name, handler);
    return () => {for (const [name, handler] of bindings) window.removeEventListener(name, handler);};
  }, []);
}
