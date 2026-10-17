import React, { useState } from "react";
import { ModelSource } from "../types/installer";
import { openFilePicker } from "../services/tauriService";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";

interface Props {
  modelSource: ModelSource;
  setModelSource: (src: ModelSource) => void;
  dllPath: string;
  setDllPath: (path: string) => void;
  binPath: string;
  setBinPath: (path: string) => void;
  hasCachedBin: boolean;
  cachedPaths?: string[];
  disabled?: boolean;
}

export const ModelSourceSelector: React.FC<Props> = ({ modelSource, setModelSource, dllPath, setDllPath, binPath, setBinPath, hasCachedBin, cachedPaths = [], disabled = false }) => {
  const { t } = useI18n();
  const [picking, setPicking] = useState(false);
  const handlePickFile = async (type: ModelSource) => {
    if (picking || disabled) return;
    setPicking(true);
    try {
      const path = await openFilePicker(type === "dll" ? "DLL/ZIP" : "Model Bin", type === "dll" ? ["dll", "zip"] : ["bin"]);
      if (path) {
        if (type === "dll") setDllPath(path);
        else setBinPath(path);
        setModelSource(type);
      }
    } catch (error) {
      console.error("Failed to select model file:", error);
    } finally { setPicking(false); }
  };

  return (
    <div className="card model-source-panel">
      <div className="model-source-heading">
        <MenuIcon name="cube" />
        <div className="model-source-heading-text">
          <h3>{t("modelSource", "title")}</h3>
        </div>
      </div>
      <p className="model-source-notice">{t("modelSource", "dllNotice")}</p>
      <div className="model-source-options">
        <button type="button" disabled={picking || disabled} aria-pressed={modelSource === "bin" && Boolean(binPath)}
          className={`model-source-option ${binPath ? "model-source-option--ready" : ""}`}
          onClick={() => handlePickFile("bin")} title={binPath || t("modelSource", "selectBin")}>
          <span className="model-source-symbol">
            <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8Z"/><path d="M14 2v6h6M8 12h2v5H8zM14 12v5m-1-5h2m-2 5h2"/></svg>
          </span>
          <span className="model-source-option-text">
            <strong>{t("modelSource", "useExisting")}</strong>
            <span>{binPath ? t("modelSource", hasCachedBin ? "binCached" : "fileSelected") : t("modelSource", "selectBin")}</span>
            {hasCachedBin && cachedPaths.length ? cachedPaths.map(path => <small key={path}>{path}</small>) : binPath && <small>{binPath}</small>}
          </span>
        </button>
        <button type="button" disabled={picking || disabled} aria-pressed={modelSource === "dll" && Boolean(dllPath)}
          className="model-source-option" onClick={() => handlePickFile("dll")} title={dllPath || t("modelSource", "selectDll")}>
          <span className="model-source-symbol"><MenuIcon name="document" /></span>
          <span className="model-source-option-text">
            <strong>{t("modelSource", "extractDll")}</strong>
            <span>{t("modelSource", "dllHint")}</span>
            {dllPath && <small>{dllPath}</small>}
          </span>
        </button>
      </div>
    </div>
  );
};
