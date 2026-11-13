import type * as React from "react";

export type AddonTarget = "settings" | "game-mods" | "game-frame-generation" | "game-captures" | "game-overview";
export type AddonIcon = "settings" | "sliders" | "graphics" | "download" | "info" | "puzzle" | "bolt" | "chip" | "play" | "folder" | "trash" | "repair";
export interface AddonContext {
  readonly target: AddonTarget;
  readonly route: string;
  readonly game?: Readonly<{id: string; directory: string; executableDirectory?: string; native?: boolean; running?:boolean}>;
}
export interface AddonViewProps {readonly context: AddonContext; readonly disabled: boolean}
export interface AddonUi {
  Shortcut: React.ComponentType<{label:string;value:string;onChange:(value:string)=>void}>;
  Categories: React.ComponentType<{label: string; active: string; onChange: (id: string) => void; categories: readonly {id: string; label: string; icon: AddonIcon; content: React.ReactNode; disabled?: boolean; disabledHint?: string}[]}>;
  Row: React.ComponentType<{icon: AddonIcon; label: string; hint: string; children: React.ReactNode}>;
  Switch: React.ComponentType<{label: string; enabled: boolean; disabled?: boolean; onToggle: () => void}>;
  Choice: React.ComponentType<{label: string; value: string; options: {value: string; label: string; disabled?: boolean}[]; disabled?: boolean; onChange: (value: string) => void; menuClassName?: string; appearance?: "default" | "compact"; showSelectedMark?: boolean}>;
  Button: React.ComponentType<{children: React.ReactNode; onClick: () => void; disabled?: boolean; primary?: boolean}>;
  Icon: React.ComponentType<{name: AddonIcon}>;
  Path: React.ComponentType<{label: string; path?: string; compact?: boolean}>;
  Progress: React.ComponentType<{progress: import("./helpers/downloadProgress").TransferProgress; label: string; onDismiss?: () => void}>;
  LaunchProfile: React.ComponentType<{gameId: string; disabled?: boolean; inline?: boolean; actionLabel?: string}>;
  Projects: React.ComponentType<{projects: readonly {name: string; url: string}[]}>;
}
export interface AddonLifecycle {
  readonly signal: AbortSignal;
  onDispose(cleanup: () => void | Promise<void>): () => void;
  timeout(callback: () => void, milliseconds: number): () => void;
  interval(callback: () => void, milliseconds: number): () => void;
  listen(target: EventTarget, type: string, callback: EventListener): () => void;
}
export interface AddonMessages {
  readonly defaultLanguage: string;
  readonly resources: Readonly<Record<string, Readonly<Record<string, string>>>>;
}
export type AddonTranslator = (key: string, values?: Readonly<Record<string, string | number | boolean>>) => string;
export interface AddonI18n {
  readonly language: string;
  createTranslator(messages: AddonMessages): AddonTranslator;
}
export interface AddonHost {
  readonly apiVersion: 1;
  readonly addon: Readonly<{id: string; version: string}>;
  readonly React: typeof React;
  readonly ui: AddonUi;
  readonly lifecycle: AddonLifecycle;
  readonly language: string;
  readonly i18n: AddonI18n;
  readonly service: {call<T = unknown>(method: string, params: unknown): Promise<T>};
  readonly game?: AddonGameActions;
  readonly styles: {add(css: string): void};
  readonly files: {openFile?(path:string):Promise<void>;machine?():Promise<{gpus:{model:string;pciAddress:string;primary:boolean}[];steam_libraries:string[]}>;openDirectory(path: string): Promise<void>; chooseFile?(label:string,extensions:string[]):Promise<string|null>; chooseDirectory?(label:string):Promise<string|null>};
  setBusy(busy: boolean): void;
}
export interface AddonGameActions {
  launch?: {
    read(gameId: string): Promise<AddonLaunchState>;
    save(gameId: string, data: Record<string, unknown>): Promise<void>;
  };
  synchronizeLaunch(gameId: string, directory: string, line: string | null): Promise<boolean>;
  saveMenuShortcut(gameId: string, key: string): Promise<void>;
  refresh(gameId: string): Promise<void>;
  chooseExecutableDirectory(gameId: string): Promise<void>;
  openGroup(group: string): void;
}
export interface AddonLaunchState {
  settings: unknown;
  gameKey?:string;gameName?:string;executablePath?:string;steamAppId?:string;directory?:string;wrappers?:{program:string;enabled:boolean}[];
  flatpak: boolean;
  gamescope: boolean;
  running: boolean;
  supported: boolean;
  options: string[];
  competing: boolean;
  executable: {bits: number | null; windows: boolean};
}
export interface AddonInstance {
  readonly apiVersion: 1;
  readonly id: string;
  readonly version: string;
  readonly views: Readonly<Record<string, (props: AddonViewProps) => React.ReactNode>>;
  dispose(): void | Promise<void>;
}
export interface AddonModule {createAddon(host: AddonHost): AddonInstance | Promise<AddonInstance>}
