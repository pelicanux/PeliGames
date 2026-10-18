import { invoke } from "@tauri-apps/api/core";
import { notifyError, notifyInfo } from "../../services/notifications";

/** Check at the action, keeping local library/mod operations independent of login. */
export async function requireNexusAccount(language: string): Promise<boolean> {
  try {
    if (await invoke<unknown | null>("load_nexus_account")) return true;
    notifyInfo("Nexus Mods", language === "en"
      ? "Connect your Nexus Mods account to use the online catalog and collections. Click here to sign in."
      : "Conecte sua conta Nexus Mods para acessar o catálogo online e as coleções. Clique aqui para fazer login.", "nexus-account");
  } catch (error) {
    notifyError("Nexus Mods", error);
  }
  return false;
}
