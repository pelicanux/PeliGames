import { invoke } from "@tauri-apps/api/core";
import { TimedFeedback } from "./TimedFeedback";
import { InstalledLaunchOptions } from "./InstalledLaunchOptions";
import { normalizeAdvanced } from "../services/advancedSettings";
import { ModernGameOptions } from "./modern/ModernGameOptions";
import { GamePerformancePanel } from "./GamePerformancePanel";
import { AdvancedGameSettings } from "./AdvancedGameSettings";
import { LauncherProgramsPanel } from "./LauncherProgramsPanel";
import { usePerformanceMode } from "./EffectsContext";
import { useEffect, useLayoutEffect, useState, useId, useRef } from "react";
import { openDirectoryPicker, openFilePicker } from "../services/tauriService";
import { updatePeliGamesSettings } from "../services/installedLibrary";
import type { GameInfo } from "./GameGrid";
import { HoverTooltip } from "./HoverTooltip";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";

/** Settings for registered PeliGames entries are independent of the mod manager. */
export type InstalledPanelSection = "performance" | "options" | "advanced";

export function InstalledGamePanels({ game, disabled, programsDisabled, onSaved, onRunProgram, onStartShortcut, section, onUninstall, onChangeExecutable }: {
  game: GameInfo | null; disabled?: boolean; onSaved: (game: GameInfo) => void;
  /** New interface: one tab's part (information, advanced settings or the
   *  execution options and programs) instead of the panel with its carousel. */
  section: InstalledPanelSection;
  onUninstall?: () => void;
  onChangeExecutable?: () => void;
  /** Programs of the prefix stay available while its game runs. */
  programsDisabled?: boolean;
  onRunProgram: (path: string, executable: string) => Promise<void>;
  onStartShortcut: (path: string, id: string) => Promise<void>;
}) {
  const { t, language } = useI18n();
  const pt = language === "pt";
  const reducedMotion = usePerformanceMode();
  const [panel, setPanel] = useState<"information" | "advanced" | "programs">("information");
  // App mounts a fresh panel for each selected game. Its first frame must already
  // contain that game's fields, so height measurement never sees an empty form.
  const [directory, setDirectory] = useState(game?.directory || game?.path || "");
  const [prefix, setPrefix] = useState(game?.prefix || "");
  const [name, setName] = useState(game?.name || "");
  const [proton, setProton] = useState(game?.proton || "");
  const [executable, setExecutable] = useState(game?.executable || (game?.external_id ? "" : game?.path) || "");
  const [busy, setBusy] = useState(false);
  const [error, setShownError] = useState("");
  const setError = (message: string) => { setShownError(message); if (message) notifyError(game?.name || t("installedSettings", "title"), message); };
  const [saved, setSaved] = useState<false | "settings" | "advanced">(false);
  const id = useId();
  const saving = useRef(false);
  useEffect(() => {
    if (section === "options" && game?.launcher === "Steam") window.dispatchEvent(new Event("steamGameOptionsOpened"));
  }, [section, game?.external_id, game?.launcher]);
  const currentGame = useRef(game);
  currentGame.current = game;
  // Saving one field must not discard unconfirmed edits in another field.
  useEffect(() => { setPanel("information"); }, [game?.path]);
  useLayoutEffect(() => { setDirectory(game?.directory || game?.path || ""); }, [game?.path, game?.directory]);
  useLayoutEffect(() => { setPrefix(game?.prefix || ""); }, [game?.path, game?.prefix]);
  useLayoutEffect(() => { setName(game?.name || ""); }, [game?.path, game?.name]);
  useLayoutEffect(() => { setProton(game?.proton || ""); }, [game?.path, game?.proton]);
  useLayoutEffect(() => { setExecutable(game?.executable || (game?.external_id ? "" : game?.path) || ""); }, [game?.path, game?.executable]);
  useEffect(() => { setError(""); setSaved(false); }, [game?.path]);
  useEffect(() => {
    if (!saved) return;
    const timeout = window.setTimeout(() => setSaved(false), 3000);
    return () => window.clearTimeout(timeout);
  }, [saved]);
  const external = Boolean(game?.external_id);
  const blocked = Boolean(disabled || busy || !game);
  const installationBlocked = blocked || external;
  const nameChanged = Boolean(game && name.trim() !== game.name);
  const executableChanged = Boolean(!external && game && executable.trim() !== (game.executable || game.path));
  const persist = async (patch: Partial<Pick<GameInfo, "name" | "proton" | "executable" | "prefix" | "advanced" | "directory">>) => {
    const entry = currentGame.current;
    if (!entry) return;
    if (entry.external_id && Object.keys(patch).some(key => key !== "advanced" && key !== "proton" && key !== "directory")) return;
    const updated = entry.external_id
      ? await invoke<GameInfo>("configure_external_game", {settings: {external_id: entry.external_id, advanced: patch.advanced ?? entry.advanced, ...(patch.proton !== undefined ? {proton: patch.proton} : {}), ...(patch.directory !== undefined ? {directory:patch.directory} : {})}})
      : await updatePeliGamesSettings({
      path: entry.path, name: entry.name, proton: entry.proton || "",
      executable: entry.executable || entry.path, prefix: entry.prefix, advanced: entry.advanced, ...patch,
    });
    const scoped = {...updated, library_view: entry.library_view, ...(entry.external_id ? {cover_url: entry.cover_url, automatic_cover_url: entry.automatic_cover_url} : {})};
    currentGame.current = scoped;
    onSaved(scoped);
    window.dispatchEvent(new Event("peligamesLibraryChanged"));
    setSaved(patch.advanced !== undefined ? "advanced" : "settings");
    if (patch.directory !== undefined) setDirectory(updated.directory || updated.path);
    if (patch.prefix !== undefined) setPrefix(updated.prefix || "");
    if (patch.name !== undefined) setName(updated.name);
    if (patch.proton !== undefined) setProton(updated.proton || "");
    if (patch.executable !== undefined) setExecutable(updated.executable || updated.path);
  };
  const save = async (patch: Partial<Pick<GameInfo, "name" | "proton" | "executable" | "prefix" | "advanced" | "directory">>) => {
    if (blocked || saving.current) return false;
    saving.current = true;
    setBusy(true); setError(""); setSaved(false);
    try { await persist(patch); return true; }
    catch (error) {
      setError(String(error));
      if (patch.proton !== undefined) setProton(currentGame.current?.proton || "");
      return false;
    } finally { saving.current = false; setBusy(false); }
  };
  const choose = async () => {
    if (blocked || saving.current || !game) return;
    saving.current = true;
    setBusy(true); setError(""); setSaved(false);
    try {
      const file = await openFilePicker(t("gameModes", "executable"), ["exe", "msi"], executable || prefix);
      if (file) await persist({ executable: file });
    } catch (error) { setError(String(error)); }
    finally { saving.current = false; setBusy(false); }
  };
  const hasPrefix = Boolean(prefix.trim());
  const maintenancePath = game?.external_id ? `external:${game.external_id}` : game?.path || "";
  const programsBlocked = Boolean(programsDisabled || !game || !hasPrefix);
  // Runs any program in this prefix, also beside the running game (Cheat Engine…).
  const runProgram = async () => {
    if (programsBlocked || !game) return;
    setError("");
    try {
      const file = await openFilePicker(t("gameModes", "executable"), ["exe", "msi"], executable);
      if (file) await onRunProgram(maintenancePath, file);
    } catch (error) { setError(String(error)); }
  };
  const choosePrefix = async () => {
    if (blocked || saving.current) return;
    try {
      const path = await openDirectoryPicker(t("installedSettings", "choosePrefix"), prefix);
      if (path && path !== currentGame.current?.prefix) await save({ prefix: path });
    } catch (error) { setError(String(error)); }
  };
  const prefixChanged = Boolean(game && prefix.trim() !== game.prefix);
  const nameField = <form className="installation-field" onSubmit={event => { event.preventDefault(); if (nameChanged && name.trim()) void save({ name: name.trim() }); }}>
            <label htmlFor={`${id}-name`}>{t("gameModes", "gameName")}</label>
            <HoverTooltip text={name} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
              <div className="game-directory-value installation-path-box"><input id={`${id}-name`} className="installation-path-input" disabled={external} value={name} onChange={event => { setName(event.target.value); setSaved(false); }} /></div>
            </HoverTooltip>
            {nameChanged && Boolean(name.trim()) && <button type="submit" className="installation-name-confirm" disabled={blocked}><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
          </form>;
  const directoryChanged = Boolean(game && directory.trim() !== (game.directory || game.path));
  const gameDirectoryField = external && <form className="installation-field" onSubmit={event => { event.preventDefault(); if (directoryChanged && directory.trim()) void save({directory:directory.trim()}); }}>
    <label htmlFor={`${id}-directory`}>{t("gameInfo", "gameDirectory")}</label>
    <div className="game-directory-value installation-path-box"><input id={`${id}-directory`} className="installation-path-input" disabled={blocked} value={directory} onChange={event => { setDirectory(event.target.value); setSaved(false); }} />
      <button type="button" className="installation-path-picker" disabled={blocked} aria-label={t("gameInfo", "editDirectory")} onClick={() => { void openDirectoryPicker(t("gameInfo", "gameDirectory"),directory).then(path=>{if(path) return save({directory:path});}).catch(error=>setError(String(error))); }}><MenuIcon name="folderSearch" /></button>
    </div>
    {directoryChanged && directory.trim() && <button type="submit" className="installation-name-confirm" disabled={blocked}><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
  </form>;
  const directoryFields = <fieldset className="installed-info-paths" disabled={installationBlocked}>

              <form className="installation-field" onSubmit={event => { event.preventDefault(); if (prefixChanged && prefix.trim()) void save({ prefix: prefix.trim() }); }}>
                <label htmlFor={`${id}-prefix`}>{t("gameInfo", "prefixDirectory")}</label>
                <HoverTooltip text={prefix} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
                  <div className="game-directory-value installation-path-box">
                    <input id={`${id}-prefix`} className="installation-path-input" value={prefix} onChange={event => { setPrefix(event.target.value); setSaved(false); }} />
                    <button type="button" className="installation-path-picker" aria-label={t("installedSettings", "choosePrefix")} onClick={() => void choosePrefix()}><MenuIcon name="folderSearch" /></button>
                  </div>
                </HoverTooltip>
                {prefixChanged && Boolean(prefix.trim()) && <button type="submit" className="installation-name-confirm" disabled={blocked}><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
              </form>
          <form className="installation-field" onSubmit={event => { event.preventDefault(); if (executableChanged && executable.trim()) void save({ executable: executable.trim() }); }}>
            <label htmlFor={`${id}-exe`}>{t("gameModes", "executable")}</label>
            <HoverTooltip text={executable} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
              <div className="game-directory-value installation-path-box">
                <input id={`${id}-exe`} className="installation-path-input" value={executable} onChange={event => { setExecutable(event.target.value); setSaved(false); }} />
                <button type="button" className="installation-path-picker" aria-label={t("gameModes", "browseExecutable")} onClick={() => void choose()}><MenuIcon name="folderSearch" /></button>
              </div>
            </HoverTooltip>
            {executableChanged && <button type="submit" className="installation-name-confirm" disabled={blocked || !executable.trim()}><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
          </form>
            </fieldset>;
  const programsPanel = game && <LauncherProgramsPanel path={game.path} disabled={programsBlocked} onStart={id => onStartShortcut(game.path, id)} />;
  const advancedPanel = <AdvancedGameSettings hideExecutionEditors key={game?.path} value={game?.advanced} disabled={blocked} reducedMotion={reducedMotion} saved={saved === "advanced"} onChange={value => void save({ advanced: value })} isSteam={game?.launcher === "Steam"} appId={game?.app_id} />;
  if (section === "advanced") return <div className="modern-advanced-tab">{advancedPanel}{error && <TimedFeedback>{error}</TimedFeedback>}</div>;
  if (section === "performance") return <GamePerformancePanel key={game?.path} value={game?.advanced} disabled={blocked} onChange={value => save({advanced: value})} />;
  const isBottles = external && game?.launcher === "Bottles";
  const protonDisabled = isBottles;
  const protonDisabledHint = isBottles
    ? (pt ? "O Bottles compartilha o runner na mesma bottle. Altere no Bottles." : "Bottles shares the runner in the same bottle. Change in Bottles.")
    : undefined;
  const protonDisabledLabel = isBottles
    ? (pt ? "Gerenciado no Bottles" : "Managed in Bottles")
    : undefined;

  if (section === "options") return game ? <><ModernGameOptions name={game.name} protonLabel={game.steam_proton_name || undefined} protonDisabled={protonDisabled} protonDisabledHint={protonDisabledHint} protonDisabledLabel={protonDisabledLabel} value={game.advanced} proton={proton} onProton={value=>save({proton:value})} onAdvanced={value=>save({advanced:value})} disabled={blocked} native={game.native_linux || game.nexus_platform === "native"} maintenancePath={game.external_id ? `external:${game.external_id}` : game.path} onUninstall={onUninstall} onChangeExecutable={onChangeExecutable}
    launchOptions={<InstalledLaunchOptions inline game={game} disabled={blocked} errorMessage={error} onSave={options=>save({advanced:{...normalizeAdvanced(currentGame.current?.advanced),launch_options:options}})}/>} configuration={
      <section className="modern-game-fields modern-options-section"><h4>{t("installedSettings", "title")}</h4><fieldset className="installation-fields" disabled={blocked}>{nameField}</fieldset>{gameDirectoryField}{directoryFields}
        {error && <TimedFeedback>{error}</TimedFeedback>}{saved === "settings" && <TimedFeedback>{t("installedSettings", "saved")}</TimedFeedback>}
      </section>} prefixPrograms={<section className="modern-prefix-actions modern-options-section"><h4>{t("installedSettings", "runTitle")}</h4>
        <div className="modern-prefix-buttons"><button type="button" className="modern-mini-btn" disabled={programsBlocked} onClick={() => void runProgram()}><MenuIcon name="play" />{t("installedSettings", "runProgram")}</button>
        <button type="button" className="modern-mini-btn" disabled={programsBlocked || external} aria-expanded={panel === "programs"} onClick={() => setPanel(value => value === "programs" ? "information" : "programs")}><MenuIcon name="gamepad" />{t("installedSettings", "programsButton")}</button>
        </div>
        {panel === "programs" && !external && programsPanel}
      </section>} /></> : null;
  return null;
}
