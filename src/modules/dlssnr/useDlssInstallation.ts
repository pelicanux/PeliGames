import { useRef, useState } from "react";
import type { GameInfo } from "../../components/GameGrid";
import { useI18n } from "../../i18n/I18nContext";
import type { InstallerPayload } from "../../types/installer";
import { runInstallation } from "../../services/installerService";
import { isMissingModel, localizeInstallationMessage } from "../../services/installationMessages";
import { launchOptionsFromInstallOutput, steamLaunchOptionsForRoute } from "../../services/routeDefaults";
import { syncModLaunchOptions } from "../../services/modLaunchOptions";
import { notifyError } from "../../services/notifications";

interface InstallationContext {
  payload: InstallerPayload;
  game: GameInfo | null;
  displayName?: string;
  onGameUpdated: (game: GameInfo) => void;
  onRefresh: () => Promise<void>;
}
interface InstallContext extends InstallationContext {
  modelConfigured: boolean;
  installed: boolean;
}
interface Result {
  title: string;
  message: string;
  type: "success" | "error";
  modelRecovery?: boolean;
  launchGame?: GameInfo;
}

function shortError(error: unknown) {
  const text = String(error);
  if (text.includes("Log completo:")) return text.split("Log completo:")[0].trim();
  const lines = text.split("\n");
  return lines.slice(0, 2).join("\n") + (lines.length > 2 ? "\n..." : "");
}

/** Owns installation feedback and confirmation; receives a frozen game context per action. */
export function useDlssInstallation() {
  const { t, language } = useI18n();
  const locked = useRef(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [logs, setLogs] = useState("");
  const [result, setResult] = useState<Result | null>(null);
  const [uninstallPath, setUninstallPath] = useState<string | null>(null);
  const appendLog = (text: string) => setLogs(previous => previous + text);

  const install = async (context: InstallContext) => {
    if (locked.current) return;
    const { payload, game, displayName, onGameUpdated, onRefresh, installed } = context;
    if (!context.modelConfigured) {
      setLogs("");
      setResult({ title: t("app", "installFailTitle"), message: t("installation", "missingModel"), type: "error", modelRecovery: true });
      return;
    }
    locked.current = true;
    setBusy(true);
    setMessage(t("app", "installingMsg"));
    setLogs(t("app", "installStarted"));
    try {
      // Paint loading feedback before entering the native installation flow.
      await new Promise(resolve => setTimeout(resolve, 50));
      const response = await runInstallation(payload);
      appendLog("\n" + localizeInstallationMessage(response, language));
      const printed = launchOptionsFromInstallOutput(response);
      const launchOptions = printed ?? steamLaunchOptionsForRoute(payload.route);
      const synced = await syncModLaunchOptions(game, payload.gameDir, launchOptions).catch(error => {
        appendLog(`\n\n${t("app", "launchOptionsError")}\n${String(error)}`);
        notifyError(displayName || "DLSS 5 NR", error);
        return undefined;
      });
      if (synced) onGameUpdated(synced);
      const layersNote = launchOptions.includes("VK_INSTANCE_LAYERS") ? `\n\n${t("routeSelector", "layersNote")}` : "";
      appendLog(`${printed ? "" : `\n\n${t("app", "launchOptionsLogTitle")}\n${launchOptions}`}${layersNote}`);
      const intro = installed ? t("app", "repairSuccessMsg") : t("app", "installSuccessMsg");
      const appliedKey = synced?.steamModOptions === "written" ? "launchOptionsSteam"
        : synced?.steamModOptions === "waiting" ? "launchOptionsSteamWaiting"
        : synced ? "launchOptionsApplied"
        : synced === undefined ? "launchOptionsError" : "launchOptionsManual";
      const launchGame = synced?.external_id && synced.launcher !== "Steam" ? synced : undefined;
      setResult({
        title: installed ? t("app", "repairSuccessTitle") : t("app", "installSuccessTitle"),
        message: launchGame ? intro : `${intro}\n\n${t("app", appliedKey)}`,
        type: "success", launchGame,
      });
    } catch (error) {
      appendLog(t("app", "criticalError") + localizeInstallationMessage(String(error), language));
      const modelRecovery = isMissingModel(error);
      setResult({
        title: t("app", "installFailTitle"), type: "error", modelRecovery,
        message: modelRecovery ? t("installation", "missingModel") : t("app", "installFailMsg") + localizeInstallationMessage(shortError(error), language),
      });
    } finally {
      try { await onRefresh(); }
      finally { locked.current = false; setBusy(false); setMessage(""); }
    }
  };

  const uninstall = async ({ payload, game, displayName, onGameUpdated, onRefresh }: InstallationContext) => {
    if (locked.current) return;
    locked.current = true;
    setBusy(true);
    setMessage(t("app", "uninstallingMsg"));
    setLogs(t("app", "uninstallStarted"));
    try {
      await new Promise(resolve => setTimeout(resolve, 50));
      const response = await runInstallation({ ...payload, route: "remove", dllPath: null, binPath: null });
      appendLog("\n" + localizeInstallationMessage(response, language));
      const synced = await syncModLaunchOptions(game, payload.gameDir, null).catch(error => { notifyError(displayName || "DLSS 5 NR", error); return null; });
      if (synced) onGameUpdated(synced);
      setResult({
        title: t("app", "uninstallSuccessTitle"), type: "success",
        message: t("app", "uninstallSuccessMsg") + payload.gameDir + (synced?.steamModOptions ? `\n\n${t("app", "launchOptionsRemovedSteam")}` : synced && synced.advanced?.launch_options !== game?.advanced?.launch_options ? `\n\n${t("app", "launchOptionsRemoved")}` : ""),
      });
    } catch (error) {
      appendLog(t("app", "criticalError") + localizeInstallationMessage(String(error), language));
      setResult({
        title: t("app", "uninstallFailTitle"), type: "error",
        message: `${t("app", "uninstallFailMsg")} ${payload.gameDir}${t("app", "uninstallFailLog")}${localizeInstallationMessage(shortError(error), language)}`,
      });
    } finally {
      setBusy(false); setMessage("");
      try { await onRefresh(); } finally { locked.current = false; }
    }
  };

  return {
    busy, message, logs, result, uninstallPath, install, uninstall,
    hasDialog: busy || Boolean(result) || uninstallPath !== null,
    requestUninstall: setUninstallPath,
    closeResult: () => setResult(null),
    showError: (title: string, message: string) => { setLogs(""); setResult({ title, message, type: "error" }); },
  };
}
