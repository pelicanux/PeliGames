import { invoke } from "@tauri-apps/api/core";
/** Pass the launcher window's activation context when opening web links. */
export function openBrowserUrl(url: string): Promise<void> {
  return invoke("open_browser_url", { url });
}
/** Nexus website session in an isolated native webview, separate from API access. */
export function openNexusBrowser(url = "https://users.nexusmods.com/"): Promise<void> {
  return invoke("open_nexus_browser", { url });
}
