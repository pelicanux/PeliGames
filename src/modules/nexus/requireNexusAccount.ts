import {translator} from "../../i18n/engine";
import { invoke } from "@tauri-apps/api/core";
import { notifyError, notifyInfo } from "../../services/notifications";

export function notifyNexusAccountRequired(language: string, reason: "catalog" | "download" = "catalog") {
  const tx = translator(language);

  const message = reason === "download"
    ? tx("requireNexusAccount", "download_not_started_connect_your_nexus_mods_account_in_peligames_click_here_to_connect_")
    : tx("requireNexusAccount", "connect_your_nexus_mods_account_to_use_the_online_catalog_and_collections_click_here_to_");
  notifyInfo("Nexus Mods", message, "nexus-account");
}

/** Check at the action, keeping local library/mod operations independent of login. */
export async function requireNexusAccount(language: string): Promise<boolean> {
  try {
    if (await invoke<unknown | null>("load_nexus_account")) return true;
    notifyNexusAccountRequired(language);
  } catch (error) {
    notifyError("Nexus Mods", error);
  }
  return false;
}
