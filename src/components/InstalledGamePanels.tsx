import { TimedFeedback } from "./TimedFeedback";
import { WinetricksModal } from "./WinetricksModal";
import { runWineTool } from "../services/wineTools";
import { AdvancedGameSettings } from "./AdvancedGameSettings";
import { AnimatePresence, motion } from "framer-motion";
import { usePerformanceMode } from "./EffectsContext";
import { useEffect, useLayoutEffect, useState, useId, useRef } from "react";
import { ProtonSelector } from "./ProtonSelector";
import { openDirectoryPicker, openFilePicker } from "../services/tauriService";
import { updatePeliGamesSettings } from "../services/installedLibrary";
import type { GameInfo } from "./GameGrid";
import { GameInfoPanel } from "./GameInfoPanel";
import { HoverTooltip } from "./HoverTooltip";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";

/** Settings for registered PeliGames entries are independent of the mod manager. */
export function InstalledGamePanels({ game, disabled, onSaved, onRunProgram }: {
  game: GameInfo | null; disabled?: boolean; onSaved: (game: GameInfo) => void;
  onRunProgram: (path: string, executable: string) => Promise<void>;
}) {
  const { t } = useI18n();
  const reducedMotion = usePerformanceMode();
  const [winetricks, setWinetricks] = useState(false);
  const [toolStatus, setToolStatus] = useState("");
  const [advanced, setAdvanced] = useState(false);
  // App mounts a fresh panel for each selected game. Its first frame must already
  // contain that game's fields, so height measurement never sees an empty form.
  const [prefix, setPrefix] = useState(game?.prefix || "");
  const [name, setName] = useState(game?.name || "");
  const [proton, setProton] = useState(game?.proton || "");
  const [executable, setExecutable] = useState(game?.executable || game?.path || "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [saved, setSaved] = useState<false | "settings" | "advanced">(false);
  const id = useId();
  const saving = useRef(false);
  const currentGame = useRef(game);
  currentGame.current = game;
  // Saving one field must not discard unconfirmed edits in another field.
  useEffect(() => { setAdvanced(false); }, [game?.path]);
  useLayoutEffect(() => { setPrefix(game?.prefix || ""); }, [game?.path, game?.prefix]);
  useLayoutEffect(() => { setName(game?.name || ""); }, [game?.path, game?.name]);
  useLayoutEffect(() => { setProton(game?.proton || ""); }, [game?.path, game?.proton]);
  useLayoutEffect(() => { setExecutable(game?.executable || game?.path || ""); }, [game?.path, game?.executable]);
  useEffect(() => { setError(""); setSaved(false); }, [game?.path]);
  useEffect(() => {
    if (!saved) return;
    const timeout = window.setTimeout(() => setSaved(false), 3000);
    return () => window.clearTimeout(timeout);
  }, [saved]);
  const blocked = Boolean(disabled || busy || !game);
  const nameChanged = Boolean(game && name.trim() !== game.name);
  const executableChanged = Boolean(game && executable.trim() !== (game.executable || game.path));
  const persist = async (patch: Partial<Pick<GameInfo, "name" | "proton" | "executable" | "prefix" | "advanced">>) => {
    const entry = currentGame.current;
    if (!entry) return;
    const updated = await updatePeliGamesSettings({
      path: entry.path, name: entry.name, proton: entry.proton || "",
      executable: entry.executable || entry.path, prefix: entry.prefix, advanced: entry.advanced, ...patch,
    });
    currentGame.current = updated;
    onSaved(updated);
    window.dispatchEvent(new Event("peligamesLibraryChanged"));
    setSaved(patch.advanced !== undefined ? "advanced" : "settings");
    if (patch.prefix !== undefined) setPrefix(updated.prefix || "");
    if (patch.name !== undefined) setName(updated.name);
    if (patch.proton !== undefined) setProton(updated.proton || "");
    if (patch.executable !== undefined) setExecutable(updated.executable || updated.path);
  };
  const save = async (patch: Partial<Pick<GameInfo, "name" | "proton" | "executable" | "prefix" | "advanced">>) => {
    if (blocked || saving.current) return;
    saving.current = true;
    setBusy(true); setError(""); setSaved(false);
    try { await persist(patch); }
    catch (error) {
      setError(String(error));
      if (patch.proton !== undefined) setProton(currentGame.current?.proton || "");
    } finally { saving.current = false; setBusy(false); }
  };
  const choose = async (run = false) => {
    if (blocked || saving.current || !game) return;
    saving.current = true;
    setBusy(true); setError(""); setSaved(false);
    try {
      const file = await openFilePicker(t("gameModes", "executable"), ["exe", "msi"], executable);
      if (file) {
        if (run) await onRunProgram(game.path, file);
        else await persist({ executable: file });
      }
    } catch (error) { setError(String(error)); }
    finally { saving.current = false; setBusy(false); }
  };
  const choosePrefix = async () => {
    if (blocked || saving.current) return;
    try {
      const path = await openDirectoryPicker(t("installedSettings", "choosePrefix"), prefix);
      if (path && path !== currentGame.current?.prefix) await save({ prefix: path });
    } catch (error) { setError(String(error)); }
  };
  const openWinecfg = async () => {
    if (blocked || !game) return;
    setBusy(true); setError(""); setToolStatus("Abrindo WineCFG…");
    try { await runWineTool(game.path,"winecfg"); setToolStatus("WineCFG encerrado."); }
    catch(e) { setError(String(e)); setToolStatus(""); } finally { setBusy(false); }
  };
  const prefixChanged = Boolean(game && prefix.trim() !== game.prefix);
  return <>
    {winetricks && game && <WinetricksModal path={game.path} name={game.name} onClose={() => setWinetricks(false)} />}
    <div className="mod-config-panel menu-glass-panel installation-config-panel">
      <div style={{ display: "flex", alignItems: "center", gap: ".5rem" }}><MenuIcon name="settings" /><span style={{ fontWeight: "bold" }}>{t("installedSettings", "title")}</span></div>
      <fieldset className="installation-fields" disabled={blocked}>
          <form className="installation-field" onSubmit={event => { event.preventDefault(); if (nameChanged && name.trim()) void save({ name: name.trim() }); }}>
            <label htmlFor={`${id}-name`}>{t("gameModes", "gameName")}</label>
            <HoverTooltip text={name} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
              <div className="game-directory-value installation-path-box"><input id={`${id}-name`} className="installation-path-input" value={name} onChange={event => { setName(event.target.value); setSaved(false); }} /></div>
            </HoverTooltip>
            {nameChanged && Boolean(name.trim()) && <button type="submit" className="installation-name-confirm" disabled={blocked}><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
          </form>
          <ProtonSelector value={proton} onChange={value => { if (value !== currentGame.current?.proton) { setProton(value); void save({ proton: value }); } }} label={t("installedSettings", "proton")} disabled={blocked} />
          <div className="installation-field">
            <span className="installation-field-label">{t("installedSettings", "runTitle")}</span>
            <HoverTooltip text={t("installedSettings", "runHint")}>
              <button type="button" className="installation-name-confirm installed-run-program" onClick={() => void choose(true)}><MenuIcon name="play" />{t("installedSettings", "runProgram")}</button>
            </HoverTooltip>
          </div>
          <button type="button" className="installation-name-confirm installed-run-program" aria-expanded={advanced}
            onClick={() => setAdvanced(value => !value)}><MenuIcon name={advanced ? "back" : "settings"} />{t("installedSettings", advanced ? "backToInfo" : "advancedButton")}</button>
          <div className="installed-wine-tools"><button type="button" className="installation-name-confirm" onClick={() => setWinetricks(true)}><MenuIcon name="cube" />Winetricks</button><button type="button" className="installation-name-confirm" onClick={() => void openWinecfg()}><MenuIcon name="wine" />WineCFG</button></div>
        </fieldset>
      {toolStatus && <TimedFeedback>{toolStatus}</TimedFeedback>}
      {error && <TimedFeedback>{error}</TimedFeedback>}
      {saved === "settings" && <TimedFeedback>{t("installedSettings", "saved")}</TimedFeedback>}
    </div>
    <div className="installed-info-carousel">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={advanced ? "advanced" : "information"} className="installed-info-slide"
          initial={{ x: reducedMotion ? 0 : "100%", opacity: reducedMotion ? 1 : 0 }}
          animate={{ x: 0, opacity: 1 }} exit={{ x: reducedMotion ? 0 : "-100%", opacity: reducedMotion ? 1 : 0 }}
          transition={{ duration: reducedMotion ? 0 : .29, ease: [0.4, 0, 0.2, 1] }}>
          {advanced ? <AdvancedGameSettings value={game?.advanced} disabled={blocked} reducedMotion={reducedMotion} saved={saved === "advanced"} onChange={value => void save({ advanced: value })} /> : <GameInfoPanel release={t("gameInfo", "unknown")} platform="Windows (Proton / Wine)"
            directoryRow={<fieldset className="installed-info-paths" disabled={blocked}>
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
            </fieldset>} />}
        </motion.div>
      </AnimatePresence>
    </div>
  </>;
}
