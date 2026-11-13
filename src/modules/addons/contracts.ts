import type {AddonTarget} from "../../../addons/_infra/ui-sdk";
export type {AddonTarget};
export type SelectionSnapshot = Readonly<Record<string, Readonly<{version: string; digest: string}>>>;
export interface AddonRoute {id: string; target: AddonTarget; route: string; title?: string; project?: {name: string; url: string}}
export type AddonCategory = "mods" | "frame-generation" | "captures";
export interface UiContribution {id: string; version: string; name: string; repository: string; category?: AddonCategory | null; group?: string | null; contribution: {target: AddonTarget; route: string; title: string; descriptions?: Record<string,string>}}
export interface AddonManifest {id:string;version:string;name:string;summary:string;repository:string;category?:AddonCategory|null;group?:string|null;contributions:UiContribution["contribution"][];dependencies:Record<string,string>}
export interface InstalledAddon {selected:string|null;enabled:boolean;versions:Record<string,{manifest:AddonManifest;consumers:string[];local?:boolean}>}
export interface AddonOverview {
  catalog:{addons:{manifest:AddonManifest;artifacts:Record<string,{bytes:number}>}[]};
  state:{selection_completed:boolean;addons:Record<string,InstalledAddon>};
  catalog_warning?:boolean;platform:string;host_version:string;contributions:UiContribution[];
  bundled_targets?: AddonTarget[];
}
export function contributionRoute(item:UiContribution):AddonRoute {return {id:item.id,...item.contribution,project:{name:item.name,url:item.repository}}}
export interface UiBundle extends AddonRoute {
  version: string; host_api: number; source: string; source_sha256: string; selection: SelectionSnapshot;
}
export function selectionCurrent(selection: SelectionSnapshot, current: SelectionSnapshot): boolean {
  return Object.entries(selection).every(([id, expected]) => current[id]?.version === expected.version && current[id]?.digest === expected.digest);
}
export function selectionKey(id: string, selection: SelectionSnapshot): string {
  return JSON.stringify([id, Object.entries(selection).sort(([a], [b]) => a.localeCompare(b))]);
}
export function closedError(): Error {return new DOMException("Add-on view is closed", "AbortError")}

export interface LocalAddonPlan {manifest:AddonManifest & {author:string;host_version:string;license:string};sha256:string;bytes:number}
