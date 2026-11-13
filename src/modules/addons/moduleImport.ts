import type {AddonModule} from "../../../addons/_infra/ui-sdk";
import type {UiBundle} from "./contracts";
/** Only the native verified source reaches this loader. Publishers bundle one ESM. */
export async function importAddonModule(bundle: UiBundle): Promise<AddonModule> {
  const url = URL.createObjectURL(new Blob([bundle.source], {type: "text/javascript"}));
  try {return await import(/* @vite-ignore */ url) as AddonModule}
  finally {URL.revokeObjectURL(url)}
}
