import type {OptiScalerMessageKey} from "./messages";
import {createContext, useContext, type ComponentType, type ReactNode} from "react";
import type {AddonUi} from "../../../_infra/ui-sdk";
import type {TransferProgress} from "../../../_infra/ui-sdk/helpers/downloadProgress";
import type {UpscalerState,UpscalerSettings,OptiScalerSoftware} from "./settings";

/** Game identity is opaque to the integration. Launcher records stay in the host. */
export interface GameReference {path: string}
export interface RuntimeState {channel:"stable"|"nightly";directory?:string|null;version:string|null;ready:boolean;software?:OptiScalerSoftware[]}
export interface OptiScalerApi {
  inspect(directory:string):Promise<UpscalerState>;
  install(directory:string,proxy:string,settings:UpscalerSettings):Promise<string>;
  upgrade(directory:string):Promise<string>;
  remove(directory:string):Promise<void>;
  update(directory:string,key:string,value:string):Promise<UpscalerSettings>;
  setPatcher(directory:string,enabled:boolean):Promise<void>;
  runtimeState():Promise<RuntimeState>;
  setChannel(channel:RuntimeState["channel"]):Promise<RuntimeState>;
  removeDownload(channel:RuntimeState["channel"]):Promise<RuntimeState>;
  download(channel:RuntimeState["channel"],onProgress:(progress:TransferProgress)=>void):Promise<void>;
}
export interface OptiScalerEnvironment {
  t(namespace: "upscaler", key: OptiScalerMessageKey, values?: Record<string, string | number>): string;
  api: OptiScalerApi;
  openDirectory(path:string):Promise<void>;
  synchronizeLaunch(game: GameReference, directory: string, line: string | null): Promise<boolean>;
  ui: Pick<AddonUi, "Categories" | "Choice" | "Switch" | "Icon"> & {
    Path: ComponentType<{label: string; path?: string; compact?: boolean}>;
    Progress: ComponentType<{progress: TransferProgress; label: string; onDismiss?: () => void}>;
    LaunchProfile: ComponentType<{game: GameReference; disabled?: boolean; inline?: boolean; actionLabel?: string}>;
    Projects: ComponentType<{addon: "optiscaler"}>;
  };
}
const Environment = createContext<OptiScalerEnvironment | undefined>(undefined);
export function OptiScalerProvider({value, children}: {value: OptiScalerEnvironment; children: ReactNode}) {
  return <Environment.Provider value={value}>{children}</Environment.Provider>;
}
export function useOptiScalerEnvironment(): OptiScalerEnvironment {
  const value = useContext(Environment);
  if (!value) throw new Error("OptiScaler requires a host connection");
  return value;
}
export const useI18n = () => ({t: useOptiScalerEnvironment().t});
export function SelectionMenu(props: React.ComponentProps<AddonUi["Choice"]>) {
  const Control = useOptiScalerEnvironment().ui.Choice;
  return <Control {...props}/>;
}
export function ModOptionSwitch({appearance: _appearance, ...props}: React.ComponentProps<AddonUi["Switch"]> & {appearance?: "rail"}) {
  const Control = useOptiScalerEnvironment().ui.Switch;
  return <Control {...props}/>;
}
export function MenuIcon(props: React.ComponentProps<AddonUi["Icon"]>) {
  const Control = useOptiScalerEnvironment().ui.Icon;
  return <Control {...props}/>;
}
export function SettingsCategories<T extends string>(props: {
  label: string; active: T; onChange: (id: T) => void;
  categories: readonly {id: T; label: string; icon: React.ComponentProps<AddonUi["Icon"]>["name"]; content: ReactNode; disabled?: boolean; disabledHint?: string}[];
}) {
  const Control = useOptiScalerEnvironment().ui.Categories;
  return <Control {...props} onChange={value => props.onChange(value as T)}/>;
}
export function GamePathTile(props: {label: string; path?: string; compact?: boolean}) {
  const Control = useOptiScalerEnvironment().ui.Path;
  return <Control {...props}/>;
}
export function ComponentDownloadProgress(props: {progress: TransferProgress; label: string; onDismiss?: () => void}) {
  const Control = useOptiScalerEnvironment().ui.Progress;
  return <Control {...props}/>;
}
export function PeliRunProfile(props: {game: GameReference; disabled?: boolean; inline?: boolean; actionLabel?: string}) {
  const Control = useOptiScalerEnvironment().ui.LaunchProfile;
  return <Control {...props}/>;
}
export function AddonProjectLinks(props: {addon: "optiscaler"}) {
  const Control = useOptiScalerEnvironment().ui.Projects;
  return <Control {...props}/>;
}
