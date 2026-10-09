import { TimedFeedback } from "./TimedFeedback";
import { useI18n } from "../i18n/I18nContext";
import { GameInfoPanel } from "./GameInfoPanel";
import { MenuIcon } from "./MenuIcon";
import { ProtonSelector } from "./ProtonSelector";
import { HoverTooltip } from "./HoverTooltip";
import type { GameInstallationResult } from "../services/gameInstallation";
import { useId, useState, type MouseEventHandler } from "react";
import { motion } from "framer-motion";
import type { InstallationGameDraft } from "../hooks/useInstallationGameDraft";
import { openDirectoryPicker, openFilePicker } from "../services/tauriService";
import { useNexusText } from "../modules/nexus/text";

interface SummaryProps {
  draft: InstallationGameDraft;
  mode?: "install" | "add" | "mods";
  onUninstall?: () => void;
  onCollapse: () => void;
  onCoverMove: MouseEventHandler<HTMLDivElement>;
  onCoverLeave: MouseEventHandler<HTMLDivElement>;
  reducedMotion: boolean;
  ready?: boolean;
  busy?: boolean;
  onInstall?: () => void;
  installationResult?: GameInstallationResult;
  installationError?: string;
  nexusAddition?: boolean;
}

export function InstallGameSummary({ draft, onCollapse, onCoverMove, onCoverLeave, reducedMotion, mode = "install", ready, busy, onInstall, installationResult, installationError, onUninstall, nexusAddition }: SummaryProps) {
  const { t } = useI18n();
  const nexusText = useNexusText();
  return <div className="game-summary game-install-summary">
    <div className="game-cover-column">
      <motion.div className="selected-cover installation-cover" onMouseMove={onCoverMove} onMouseLeave={onCoverLeave}
        layout={reducedMotion ? false : "preserve-aspect"}>
        <div className="glare" />
        {mode !== "mods" && draft.coverUrl ? <img className="installation-cover-image" src={draft.coverUrl} alt={draft.name}
          onError={() => draft.coverUrl && draft.coverFailed(draft.coverUrl)} /> : <div className="installation-cover-placeholder">
          <img src="/peligames.svg" alt="Icon" />
        </div>}
      </motion.div>
      <button type="button" disabled={busy} className="btn game-back-button installation-collapse" onClick={onCollapse}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg>
        {t("gameInfo", "backToGrid")}
      </button>
    </div>
    <div className="game-action-column">
      <h3 className="selected-game-title">{mode === "mods" ? t("gameModes", "mods") : draft.name || t("gameModes", "genericName")}</h3>
      {mode !== "mods" && draft.coverStatus === "loading" && <TimedFeedback>{t("gameModes", "searchingCover")}</TimedFeedback>}
      {mode !== "mods" && (draft.coverStatus === "missing" || draft.coverStatus === "error") && <TimedFeedback>{t("gameModes", draft.coverStatus === "missing" ? "coverNotFound" : "coverSearchFailed")}</TimedFeedback>}
      <div className="game-action-stack"><div className="game-install-action">
        <button type="button" disabled={mode === "mods" || !ready || busy} onClick={onInstall} className="primary installation-submit" aria-busy={busy}><MenuIcon name="download" />{mode === "mods" ? t("gameModes", "installMod") : busy ? t("gameModes", mode === "add" ? "adding" : "installerRunning") : mode === "add" ? t("gameModes", "add") : t("installAction", "install")}</button>
      </div>{!nexusAddition && <button type="button" disabled={busy || !onUninstall} onClick={onUninstall} className="btn btn-secondary game-uninstall-button"><MenuIcon name="trash" />{t("gameInfo", "uninstall")}</button>}</div>
      {mode === "mods" && <TimedFeedback>{t("gameModes", "selectGameHint")}</TimedFeedback>}
      {mode !== "mods" && <TimedFeedback as="div" className="installation-process-feedback" resetKey={JSON.stringify([mode, busy, ready, installationError, installationResult])}>
        {busy && <p>{t("gameModes", mode === "add" ? "adding" : "installerWait")}</p>}
        {!busy && !ready && !installationError && !installationResult && <p>{nexusAddition ? nexusText.addRequired : t("gameModes", mode === "add" ? "addRequired" : "installerRequired")}</p>}
        {installationError && <p>{installationError}</p>}
        {installationResult && <><p>{t("gameModes", "installerClosed")}</p>
          <p>{installationResult.library_error ? `${t("gameModes", "librarySaveError")} ${installationResult.library_error}` : installationResult.registered_count ? `${installationResult.registered_count} ${t("gameModes", "libraryAdded")}` : t("gameModes", "libraryNone")}</p>
          <HoverTooltip text={installationResult.prefix} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip"><p className="installation-process-path">{t("gameModes", "prefixLocation")} {installationResult.prefix}</p></HoverTooltip>
          <HoverTooltip text={installationResult.log} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip"><p className="installation-process-path">Log: {installationResult.log}</p></HoverTooltip>
        </>}
      </TimedFeedback>}
    </div>
  </div>;
}

function InstallationName({ draft }: { draft: InstallationGameDraft }) {
  const { t } = useI18n();
  const id = useId();
  const text = draft.editing ? draft.draftName : draft.name;
  const canConfirmName = Boolean(draft.draftName.trim() && draft.draftName.trim() !== draft.name);
  return <div className="installation-name-area">
    {draft.editing ? <form onSubmit={event => { event.preventDefault(); draft.confirm(); }}>
      <label className="installation-name-label">
        <span>{t("gameModes", "gameName")}</span>
        <input autoFocus className="installation-input installation-name-input" value={draft.draftName}
          aria-describedby={text.trim() ? id : undefined} onChange={event => draft.setDraftName(event.target.value)}
          placeholder={t("gameModes", "namePlaceholder")} />
      </label>
      {(canConfirmName || draft.name) && <div className="installation-name-actions">
        {canConfirmName && <button type="submit" className="installation-name-confirm"><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
        {draft.name && <button type="button" className="installation-name-cancel" onClick={draft.cancelEdit}>{t("gameModes", "cancelName")}</button>}
      </div>}
    </form> : <div className="installation-field">
      <span className="installation-field-label">{t("gameModes", "gameName")}</span>
      <div className="installation-confirmed-name game-directory-value">
        <span className="game-path-text" onClick={draft.beginEdit}>{draft.name}</span>
        <button type="button" className="installation-name-edit" onClick={draft.beginEdit} aria-label={t("gameModes", "editName")}><MenuIcon name="edit" /></button>
      </div>
    </div>}
    {text.trim() && <span role="tooltip" id={id} className="installation-name-hint">{text}</span>}
  </div>;
}

function InstallationPath({ value, onChange, executable = false, selected = true, onDefault, label, nativeExecutable = false, existingGame = false, hint }: { value: string; onChange: (path: string) => void; executable?: boolean; selected?: boolean; onDefault?: () => Promise<void>; label?: string; nativeExecutable?: boolean; existingGame?: boolean; hint?: string }) {
  const { t } = useI18n();
  const id = useId();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(false);
  const pick = async () => {
    setBusy(true); setError(false);
    try {
      const path = executable ? await openFilePicker(t("gameModes", "executable"), nativeExecutable ? ["*"] : ["exe", "msi"], existingGame ? value : undefined)
        : await openDirectoryPicker(label || t("gameModes", "installationLocation"), existingGame ? value : undefined);
      if (path) onChange(path);
    } catch { setError(true); }
    finally { setBusy(false); }
  };
  return <div className="installation-field">
    {hint ? <div className="installation-type-label">
      <label htmlFor={id} className="installation-field-label">{label || t("gameModes", "installationLocation")}</label>
      <HoverTooltip text={hint}>
        <button type="button" className="installation-type-info" aria-label={label}>
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
            <circle cx="12" cy="12" r="9" /><path d="M12 11v6M12 7h.01" />
          </svg>
        </button>
      </HoverTooltip>
    </div> : <label htmlFor={id}>{label || t("gameModes", executable ? "executable" : "installationLocation")}</label>}
    {!executable && !selected && <div className="installation-location-actions">
      {onDefault && <button type="button" className="installation-name-confirm" disabled={busy} onClick={async () => {
        setBusy(true); setError(false);
        try { await onDefault?.(); } catch { setError(true); } finally { setBusy(false); }
      }}>{t("gameModes", "defaultLocation")}</button>}
      <button type="button" className="installation-name-confirm" disabled={busy} onClick={() => void pick()}>{t("gameModes", "chooseLocation")}</button>
    </div>}
    {selected && <HoverTooltip text={value} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
    <div className="game-directory-value installation-path-box">
      <input id={id} className="installation-path-input" value={value} onChange={event => onChange(event.target.value)}
        placeholder={label || t("gameModes", executable ? "executablePlaceholder" : "directoryPlaceholder")} />
      <button type="button" className="installation-path-picker" onClick={() => void pick()} disabled={busy}
        aria-label={label || t("gameModes", executable ? "browseExecutable" : "browseDirectory")}><MenuIcon name="folderSearch" /></button>
    </div>
    </HoverTooltip>}
    {error && <TimedFeedback>{t("gameModes", selected ? "pickerError" : "locationSelectionError")}</TimedFeedback>}
  </div>;
}

interface PanelsProps {
  mode: "install" | "add";
  draft: InstallationGameDraft;
  directory: string;
  directoryError?: boolean;
  directorySelected: boolean;
  onDefaultDirectory: () => Promise<void>;
  busy?: boolean;
  onDirectoryChange: (directory: string) => void;
  executable: string;
  onExecutableChange: (path: string) => void;
  proton: string;
  onProtonChange: (proton: string) => void;
  nexusAddition?: { platform: "native" | "proton"; onPlatformChange: (platform: "native" | "proton") => void; prefix: string; onPrefixChange: (prefix: string) => void };
}

export function InstallGamePanels({ mode, draft, directory, directoryError, directorySelected, onDefaultDirectory, busy, onDirectoryChange, executable, onExecutableChange, proton, onProtonChange, nexusAddition }: PanelsProps) {
  const { t } = useI18n();
  const nexusText = useNexusText();
  return <>
    <div className={`mod-config-panel menu-glass-panel installation-config-panel ${nexusAddition ? "installation-config-panel--nexus-add" : ""}`}>
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem" }}><MenuIcon name="settings" />
        <span style={{ color: "var(--tone-ffffff, #fff)", fontWeight: "bold", fontSize: "0.95rem" }}>{nexusAddition ? nexusText.gameSettings : t("gameModes", "installationSettings")}</span>
      </div>
      <fieldset className="installation-fields" disabled={busy}>
      {nexusAddition ? <>
        <div className="installation-fields-section">
          <InstallationName draft={draft} />
          {nexusAddition.platform === "proton" && <>
            <InstallationPath value={nexusAddition.prefix} onChange={nexusAddition.onPrefixChange}
              selected={Boolean(nexusAddition.prefix)} label={`${t("gameModes", "installationLocation")} (${nexusText.prefix})`} hint={nexusText.prefixAdditionHint} existingGame />
            <ProtonSelector value={proton} onChange={onProtonChange} label={t("gameModes", "runWithProton")} />
          </>}
          <InstallationPath value={executable} onChange={onExecutableChange} executable nativeExecutable={nexusAddition.platform === "native"} existingGame />
        </div>
        <div className="installation-fields-section">
          <InstallationPath value={directory} onChange={onDirectoryChange} selected={directorySelected} label={nexusText.directory} existingGame />
          {directoryError && <TimedFeedback>{t("gameModes", "defaultDirectoryError")}</TimedFeedback>}
          <fieldset className="nexus-platform"><legend>{nexusText.platform}</legend>{(["native", "proton"] as const).map(platform => <label key={platform}><input type="radio" name="nexus-add-platform" checked={nexusAddition.platform === platform} onChange={() => nexusAddition.onPlatformChange(platform)} />{nexusText[platform]}</label>)}</fieldset>
        </div>
      </> : <>
        <InstallationName draft={draft} />
        <InstallationPath value={directory} onChange={onDirectoryChange} selected={directorySelected} onDefault={onDefaultDirectory} />
        {directoryError && <TimedFeedback>{t("gameModes", "defaultDirectoryError")}</TimedFeedback>}
        <ProtonSelector value={proton} onChange={onProtonChange} label={t("gameModes", mode === "add" ? "runWithProton" : "installWithProton")} />
        <InstallationPath value={executable} onChange={onExecutableChange} executable />
      </>}
      </fieldset>
    </div>
    {!nexusAddition && <GameInfoPanel release={t("gameInfo", "unknown")} platform="Windows (Proton / Wine)"
      directoryRow={<div className="game-info-directory-row">
        <span className="game-info-label"><MenuIcon name="folder" />{t("gameInfo", "directory")}</span>
        <HoverTooltip text={directory} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
        <div className="game-directory-value"><span className="game-path-text">{directory || t("gameModes", "directoryPlaceholder")}</span>
          <button type="button" disabled={busy || !directorySelected} className="installation-path-picker" aria-label={t("gameInfo", "editDirectory")}
            onClick={() => document.querySelector<HTMLInputElement>(".installation-path-input")?.focus()}><MenuIcon name="edit" /></button>
        </div>
        </HoverTooltip>
      </div>} />}
  </>;
}
