import { invoke } from "@tauri-apps/api/core";
import { notifyError, notifyInfo } from "../../services/notifications";

export function notifyNexusAccountRequired(language: string, reason: "catalog" | "download" = "catalog") {
  const message = reason === "download"
    ? language === "en"
      ? "Download not started: connect your Nexus Mods account in PeliGames. Click here to connect it, then retry the download."
      : "O download não foi iniciado: conecte sua conta Nexus Mods no PeliGames. Clique aqui para conectar e depois tente baixar novamente."
    : language === "en"
      ? "Connect your Nexus Mods account to use the online catalog and collections. Click here to sign in."
      : "Conecte sua conta Nexus Mods para acessar o catálogo online e as coleções. Clique aqui para fazer login.";
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
