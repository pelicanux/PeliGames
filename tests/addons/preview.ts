import manifest from "../../addons/optiscaler/manifest.json";
import type {createUpscalerPreview} from "../upscaler/preview";
import type {AddonManifest} from "../../src/modules/addons/contracts";
export interface PreviewAddon {manifest:AddonManifest; call(method:string,request:Record<string,unknown>):unknown}

/** Browser-preview fixture; no downloads, native processes or real game writes. */
export function createAddonPreview(upscaler: ReturnType<typeof createUpscalerPreview>, origin: string, extra:PreviewAddon[] = []) {
  let completed = false, nextJob = 0;
  const jobs = new Map<number, unknown>();
  const manifests = [manifest as AddonManifest,...extra.map(item=>item.manifest)];
  const packages = new Map(manifests.map(item=>[item.id,{manifest:item,installed:true,enabled:true,local:false}]));
  const active = () => [...packages.values()].filter(item=>item.installed&&item.enabled);
  const selection = () => Object.fromEntries(active().map(({manifest})=>[manifest.id,{version:manifest.version,digest:"a".repeat(64)}]));
  const contributions = () => active().flatMap(({manifest})=>manifest.contributions.map(contribution => ({
    id:manifest.id,version:manifest.version,name:manifest.name,repository:manifest.repository,category:manifest.category,contribution,
  })));
  const overview = () => ({
    catalog: {schema_version: 1, addons: manifests.map(manifest=>({manifest,artifacts:{}}))},
    state: {selection_completed: completed, addons: Object.fromEntries([...packages.values()].filter(item=>item.installed).map(({manifest,enabled})=>[manifest.id,{selected:manifest.version,enabled,versions:{[manifest.version]:{manifest,consumers:[],local:packages.get(manifest.id)?.local??false}}}]))},
    platform: "linux-x86_64", host_version: "0.5.5", contributions: contributions(),
  });
  const done = () => {const id = ++nextJob; jobs.set(id, {id, done: true, result: null, progress: {phase: "complete", downloaded: 100, total: 100}}); return {job: id}};
  const invoke = (command: string, args: Record<string, unknown>) => {
    if (command === "addon_store_state" || command === "addon_refresh_catalog") return overview();
    if(command==="addon_local_plan")return {manifest:{...manifest,id:"local-experiment",name:"Local experiment",version:"0.1.0",author:"Preview",host_version:">=0.5.5",license:"GPL-3.0-only",contributions:[]},sha256:"c".repeat(64),bytes:150};
    if(command==="addon_import_local"){
      if(args.sha256!=="c".repeat(64))throw Error("The local package changed");
      const local={...manifest,id:"local-experiment",name:"Local experiment",version:"0.1.0",contributions:[]} as AddonManifest;
      packages.set(local.id,{manifest:local,installed:true,enabled:true,local:true});return overview().state;
    }
    if (command === "addon_service_status") return [];
    if(command==="addon_game_refresh")return null;
    if(command==="addon_launch_settings")return (args.advanced as {addon_launch?:unknown})?.addon_launch??{};
    if(command==="addon_machine_context")return {gpus:[{model:"Preview Radeon",pciAddress:"0000:01:00.0",primary:true}],steam_libraries:["/preview/Steam"]};
    if (command === "addon_game_probe") return {bits:64,windows:true};
    if (command === "addon_ui_contributions") return contributions();
    if (command === "addon_complete_selection") {completed = true; return overview().state;}
    if (command === "addon_set_enabled" || command === "addon_remove_version") {
      const item=packages.get(String(args.id));if(!item)throw Error("Unknown preview add-on");
      if(command === "addon_remove_version"){item.installed=false;item.enabled=false;}else item.enabled=Boolean(args.enabled);
      return overview().state;
    }
    if (command === "addon_select_version") return overview().state;
    if (command === "addon_service_stop") return null;
    if (command === "addon_ui_load") {
      const item=packages.get(String(args.id));
      if(!item?.installed||!item.enabled||!item.manifest.contributions.some(item=>item.target===args.target&&item.route===args.route))throw Error("Unavailable preview add-on route");
      const manifest=item.manifest;
      return {id: manifest.id, version: manifest.version, host_api: 1, target: args.target, route: args.route,
        selection: selection(), source_sha256: "b".repeat(64),
        source: `export {createAddon} from ${JSON.stringify(new URL(`/addons/${manifest.id}/ui/src/index.tsx`, origin).href)};`,
      };
    }
    if (command === "addon_service_call") {
      const item=packages.get(String(args.id));
      if(!item?.installed||!item.enabled)throw Error("Unavailable preview add-on");
      const request = (args.params ?? {}) as Record<string, unknown>;
      const adapter=extra.find(adapter=>adapter.manifest.id===args.id);
      if(adapter)return adapter.call(String(args.method),request);
      const directory = String(request.directory ?? "");
      switch (args.method) {
        case "game.inspect": return upscaler.state(directory);
        case "runtime.inspect": return upscaler.runtimeState();
        case "runtime.channel": return upscaler.setChannel(String(request.channel));
        case "runtime.remove": return upscaler.removeDownload(String(request.channel));
        case "runtime.download": upscaler.download(); return done();
        case "job.inspect": return jobs.get(Number(request.id));
        case "game.mutate": {
          const action = request.action as Record<string, unknown>;
          switch (action.operation) {
            case "install": return upscaler.install(directory, String(action.proxy), action.settings as Record<string, string>);
            case "upgrade": return upscaler.upgrade(directory);
            case "remove": upscaler.remove(directory); return null;
            case "update": return upscaler.update(directory, String(action.key), String(action.value));
            case "patcher": upscaler.patcher(directory, Boolean(action.enabled)); return done();
          }
        }
      }
    }
    throw Error(`Unsupported preview add-on request: ${command}`);
  };
  return {invoke, overview};
}
