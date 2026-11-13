import type {ReactNode} from "react";
import {invoke} from "@tauri-apps/api/core";
import {useI18n} from "../../i18n/I18nContext";
import {notifyError} from "../../services/notifications";
import {isKnownGameInfo} from "./gameInfoPresentation";

export function GamePathTile({label, path, action, compact=false}: {label: string; path?: string; action?: ReactNode; compact?: boolean}) {
  const {t} = useI18n();
  if (!isKnownGameInfo(path)) return null;
  return <div className={compact ? "game-path-inline" : "modern-tile"}><small>{label}</small><div className="modern-path"><code title={path}>{path}</code>{action}<button className="modern-mini-btn" onClick={() => void invoke("open_folder", {path}).catch(error => notifyError(label, String(error)))}>{t("ui-app", "open")}</button><button className="modern-mini-btn" onClick={() => void invoke("plugin:clipboard-manager|write_text", {text: path}).catch(error => notifyError(label, String(error)))}>{t("ui-app", "copy")}</button></div></div>;
}
