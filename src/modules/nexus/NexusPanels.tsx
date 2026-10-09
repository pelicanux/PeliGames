import { NexusDeployOptions } from "./NexusDeployOptions";
import { nexusArchiveFormats } from "./archiveFormats";
import { useEffect, useId, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusPopup } from "./NexusPopup";
import { NexusRequirements } from "./NexusRequirements";
import { ProtonSelector } from "../../components/ProtonSelector";
import { HoverTooltip } from "../../components/HoverTooltip";
import { MenuIcon } from "../../components/MenuIcon";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { NexusInstallSequence } from "./NexusInstallSequence";
import { NexusModuleStatus } from "./NexusModuleStatus";
import { NexusCatalogPanel } from "./NexusCatalogPanel";
import { useNexusText } from "./text";
import type { NexusGame, LocalMod } from "./useNexusWorkspace";
import { modDisplay } from "./modDisplay";
export function NexusPanels({ tab, onTabChange, highlightedModId, highlightRequestId, game, busy, settingsBusy, error, reducedMotion, onImport, onRemove, onInstall, onToggle, onLogs, onConfigure, onConfigureDeploy, onAssociate, onError }: {
  tab: "mods" | "catalog"; onTabChange: (tab: "mods" | "catalog") => void;
  highlightedModId?: string; highlightRequestId?: number;
  game?: NexusGame; busy: boolean; settingsBusy: boolean; error: string; reducedMotion: boolean;
  onImport: (gameId: string, source: string) => Promise<boolean>;
  onInstall: (gameId: string, modId: string, selection?: string[]) => Promise<boolean>;
  onToggle: (gameId: string, modId: string, enabled: boolean) => Promise<boolean>;
  onConfigure: (gameId: string, compatData: string, proton: string, directory: string) => Promise<boolean>;
  onConfigureDeploy: (gameId: string, method: import("./useNexusWorkspace").DeployMethod) => Promise<boolean>;
  onAssociate: (gameId: string, domain: string) => Promise<boolean>;
  onLogs: (gameId: string) => void;
  onRemove: (gameId: string, modId: string) => Promise<boolean>; onError: (error: string) => void;
}) {
  const text = useNexusText();
  const modList = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const searchId = useId();
  const [searchOpen, setSearchOpen] = useState(false);
  const [modQuery, setModQuery] = useState("");
  const [needsModSearch, setNeedsModSearch] = useState(false);
  const [expandedModId, setExpandedModId] = useState<string | null>(null);
  const [labels, setLabels] = useState<Record<string, { name: string; version: string }>>({});
  const display = (mod: LocalMod) => labels[mod.id] || modDisplay(mod);
  const visibleMods = (game?.mods || []).filter(mod => display(mod).name.toLocaleLowerCase().includes(modQuery.trim().toLocaleLowerCase()));
  const unresolvedLabels = game?.mods.filter(mod => !mod.version || (mod.nexus_source && !mod.nexus_source.name)).map(mod => mod.id).join(",") || "";
  useEffect(() => { setExpandedModId(null); setLabels({}); }, [game?.id]);
  useEffect(() => {
    if (!game || !unresolvedLabels) return;
    let active = true;
    void invoke<LocalMod[]>("refresh_nexus_mod_labels", { gameId: game.id }).then(mods => {
      if (active) setLabels(Object.fromEntries(mods.map(mod => [mod.id, modDisplay(mod)])));
    }).catch(() => {});
    return () => { active = false; };
  }, [game?.id, unresolvedLabels]);
  useEffect(() => {
    if (!expandedModId) return;
    const outside = (event: PointerEvent) => {
      const row = Array.from(modList.current?.children || []).find(row => (row as HTMLElement).dataset.modId === expandedModId);
      if (!row?.contains(event.target as Node)) setExpandedModId(null);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [expandedModId]);
  useEffect(() => { setSearchOpen(false); setModQuery(""); setNeedsModSearch(false); }, [game?.id]);
  useEffect(() => { if (searchOpen) searchInput.current?.focus(); }, [searchOpen]);
  useEffect(() => {
    if (modQuery.trim()) return; // Keep the search control available while filtering.
    const list = modList.current;
    if (!list) { setNeedsModSearch(false); return; }
    const measure = () => setNeedsModSearch(list.scrollHeight > list.clientHeight + 1);
    const observer = new ResizeObserver(measure);
    observer.observe(list);
    for (const row of Array.from(list.children)) observer.observe(row);
    measure();
    return () => observer.disconnect();
  }, [game?.id, game?.mods, modQuery, searchOpen]);
  useEffect(() => { if (highlightedModId) { setModQuery(""); setSearchOpen(false); } }, [highlightedModId, highlightRequestId]);
  useEffect(() => { modList.current?.scrollTo({top: 0}); }, [modQuery]);
  const highlightedRow = useRef<HTMLDivElement>(null);
  const [activeHighlight, setActiveHighlight] = useState<string>();
  const highlightedModExists = Boolean(game?.mods.some(mod => mod.id === highlightedModId));
  useEffect(() => {
    setActiveHighlight(undefined);
    if (tab !== "mods" || !highlightedModId || !highlightedModExists) return;
    setActiveHighlight(highlightedModId);
    const frame = window.requestAnimationFrame(() => {
      const row = highlightedRow.current, list = row?.parentElement;
      if (row && list) {
        const target = row.getBoundingClientRect(), viewport = list.getBoundingClientRect();
        const delta = target.top < viewport.top ? target.top - viewport.top : target.bottom > viewport.bottom ? target.bottom - viewport.bottom : 0;
        if (delta) list.scrollTo({top: list.scrollTop + delta, behavior: reducedMotion ? "instant" : "smooth"});
        row.focus({preventScroll: true});
      }
    });
    const timer = window.setTimeout(() => setActiveHighlight(undefined), 3000);
    return () => { window.cancelAnimationFrame(frame); window.clearTimeout(timer); };
  }, [highlightedModId, highlightRequestId, highlightedModExists, tab, game?.id, reducedMotion]);
  const setTab = onTabChange;
  const [installReview, setInstallReview] = useState<string | null>(null);
  const [requirementsMod, setRequirementsMod] = useState<string | null>(null);
  const [choosing, setChoosing] = useState(false);
  const [removeTarget, setRemoveTarget] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  const removalLock = useRef(false);
  useEffect(() => { setTab("mods"); setInstallReview(null); setRequirementsMod(null); setRemoveTarget(null); }, [game?.id]);
  const [compatData, setCompatData] = useState("");
  const [directory, setDirectory] = useState("");
  const [proton, setProton] = useState("");
  useEffect(() => { setDirectory(game?.game.directory || ""); setCompatData(game?.compat_data || game?.game.prefix || ""); setProton(game?.proton || ""); }, [game?.id, game?.game.directory, game?.game.prefix, game?.compat_data, game?.proton]);
  const [settingsSaved, setSettingsSaved] = useState(false);
  const savedTimer = useRef<number | undefined>(undefined);
  const activeGame = useRef(game?.id); activeGame.current = game?.id;
  useEffect(() => { setSettingsSaved(false); window.clearTimeout(savedTimer.current); return () => window.clearTimeout(savedTimer.current); }, [game?.id]);
  const showSaved = () => { setSettingsSaved(true); window.clearTimeout(savedTimer.current); savedTimer.current = window.setTimeout(() => setSettingsSaved(false), 3000); };
  const saveSettings = async (patch: {directory?: string; prefix?: string; proton?: string} = {}) => {
    if (!game || busy || settingsBusy || game.running) return;
    const nextDirectory = (patch.directory ?? directory).trim(), nextPrefix = (patch.prefix ?? compatData).trim(), nextProton = patch.proton ?? proton;
    if (nextDirectory === game.game.directory && nextPrefix === (game.compat_data || game.game.prefix || "") && nextProton === (game.proton || "")) return;
    const gameId = game.id;
    setSettingsSaved(false);
    if (await onConfigure(gameId, nextPrefix, nextProton, nextDirectory) && activeGame.current === gameId) showSaved();
  };
  const [tools, setTools] = useState<{ name: string }[]>([]);
  useEffect(() => {
    let active = true; setTools([]);
    if (game?.adapter) void invoke<{ definitions: { domain: string; tools?: {name:string}[] }[] }>("list_nexus_modules")
      .then(registry => { if (active) setTools((registry?.definitions || []).find(module => module.domain === game.adapter)?.tools || []); })
      .catch(error => { if (active) onError(String(error)); });
    return () => { active = false; };
  }, [game?.id, game?.adapter]);
  const supported = Boolean(game?.adapter);
  const locked = busy || Boolean(game?.running);
  const settingsLocked = locked || settingsBusy;
  const remove = (modId: string) => {
    if (!game || locked) return;
    onError(""); setRemoveTarget(modId);
  };
  const confirmRemoval = async () => {
    if (!game || !removeTarget || locked || settingsBusy || removalLock.current) return;
    const gameId = game.id, modId = removeTarget;
    removalLock.current = true; setRemoving(true);
    try { if (await onRemove(gameId, modId) && activeGame.current === gameId) setRemoveTarget(null); }
    catch (e) { onError(String(e)); }
    finally { removalLock.current = false; setRemoving(false); }
  };
  const modToRemove = game?.mods.find(mod => mod.id === removeTarget);
  const importArchive = async () => {
    if (!game || busy || choosing) return;
    setChoosing(true);
    try { const source = await openFilePicker("Mods", nexusArchiveFormats); if (source) await onImport(game.id, source); }
    catch (e) { onError(String(e)); } finally { setChoosing(false); }
  };
  return <div className="nexus-panel">
    {modToRemove && <NexusPopup className="nexus-remove-popup" title={text.remove} onClose={() => { if (!removalLock.current) setRemoveTarget(null); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={removing} onClick={() => setRemoveTarget(null)}>{text.cancel}</button>
      <button type="button" className="btn btn-primary" disabled={locked || settingsBusy || removing} onClick={() => void confirmRemoval()}><MenuIcon name="trash" />{removing ? text.loading : text.remove}</button>
    </>}><strong className="nexus-remove-name">{display(modToRemove).name}</strong><p className="nexus-hint">{text.removePrompt}</p>{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>}
    {error && <p className="nexus-error" role="alert">{error}</p>}
    <div className="nexus-carousel-panels">
    <AnimatePresence mode="wait" initial={false}><motion.div key="mods" role="region" aria-label={text.mods} className="nexus-tab-content menu-glass-panel nexus-mods-content" initial={{ opacity: reducedMotion ? 1 : 0, y: reducedMotion ? 0 : 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: reducedMotion ? 1 : 0 }} transition={{ duration: reducedMotion ? 0 : .2 }}>
      <><div className="nexus-panel-heading"><h3><img className="nexus-icon" src="/nexus-mods.svg" alt="" />{text.title}</h3>{Boolean(game?.mods.length) && (needsModSearch || searchOpen) && <HoverTooltip text={text.searchInstalledMods} anchorClassName="nexus-mod-search-trigger"><button type="button" className="nexus-mod-search-icon" aria-label={text.searchInstalledMods} aria-expanded={searchOpen} aria-controls={searchId} onClick={() => { setSearchOpen(value => !value); setModQuery(""); }}><MenuIcon name="search" /></button></HoverTooltip>}{Boolean(game?.mods.length) && <button type="button" className="btn btn-primary" disabled={!game || busy} onClick={() => setTab("catalog")}><MenuIcon name="puzzle" />{text.installMods}</button>}</div>
        {tools.length > 0 && <div className="nexus-tabs">{tools.map(tool => <button type="button" className="btn btn-secondary" key={tool.name} disabled={!game || locked} onClick={() => { if (game) void invoke("open_nexus_game_tool", {gameId:game.id,name:tool.name}).catch(error => onError(String(error))); }}><MenuIcon name="settings" />{text.openManager}: {tool.name}</button>)}</div>}
        {["dragonageinquisition", "starwarsbattlefront22017"].includes(game?.adapter || "") && <p className="nexus-hint">{text.managerRequired}</p>}
        {searchOpen && <label className="nexus-mod-search-field"><MenuIcon name="search" /><input ref={searchInput} id={searchId} type="search" aria-label={text.searchInstalledMods} placeholder={text.searchInstalledMods} value={modQuery} onChange={event => setModQuery(event.target.value)} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); setModQuery(""); setSearchOpen(false); } }} /></label>}
        {!game?.mods.length ? <div className="nexus-empty-mods nexus-install-start"><button type="button" className="nexus-install-puzzle" aria-label={text.installFromCatalog} disabled={!game || busy} onClick={() => setTab("catalog")}><span className="nexus-install-piece"><MenuIcon name="puzzle" /></span><span className="nexus-install-download"><MenuIcon name="download" /></span></button><span>{text.installFromCatalog}</span></div> : <div ref={modList} className="nexus-mod-list" data-scrollbar-outside="true">{visibleMods.map(mod => {
          const identity = display(mod), expanded = !mod.installed || expandedModId === mod.id;
          const state = mod.installed ? mod.enabled ? "active" : "inactive" : "pending";
          return <div className={`nexus-mod-row nexus-installed-mod-row nexus-mod-${state} ${expanded ? "expanded" : "collapsed"} ${activeHighlight === mod.id ? "nexus-mod-highlighted" : ""}`} data-mod-id={mod.id} ref={highlightedModId === mod.id ? highlightedRow : undefined} tabIndex={-1} key={mod.id}>
            <button type="button" className="nexus-mod-summary" aria-expanded={expanded} aria-controls={`mod-actions-${mod.id}`} onClick={() => { if (mod.installed) setExpandedModId(current => current === mod.id ? null : mod.id); }}>
              <span className="nexus-installed-mod-info"><span className="nexus-mod-title"><strong>{identity.name}</strong>{expanded && identity.version && <span className="nexus-mod-version">v{identity.version}</span>}</span>{expanded && <small>{mod.archive.split(".").pop()?.toUpperCase()} · {mod.installed ? mod.enabled ? ["dragonageinquisition", "starwarsbattlefront22017"].includes(game?.adapter || "") ? text.preparedForManager : text.enabled : text.disabled : text.imported} · {(mod.size / 1024 / 1024).toFixed(1)} MB</small>}</span>
              {mod.installed && <span className={`nexus-mod-chevron ${expanded ? "expanded" : ""}`} aria-hidden="true">⌄</span>}
            </button>
            {expanded && mod.identification_note && <small className="nexus-local-identification-note" role="status">{mod.identification_note}</small>}
            {expanded && <div id={`mod-actions-${mod.id}`} className="nexus-installed-mod-actions">
              <button type="button" className="btn btn-secondary" disabled={locked || (mod.installed && !supported)} onClick={() => { if (mod.installed) void onToggle(game.id, mod.id, !mod.enabled).then(ok => { if (ok) setExpandedModId(current => current === mod.id ? null : current); }); else setInstallReview(mod.id); }}><MenuIcon name={mod.installed ? "check" : "download"} />{mod.installed ? mod.enabled ? text.disable : text.enable : text.install}</button>
              {mod.nexus_source && <button type="button" className="btn btn-secondary nexus-requirements-link" onClick={() => setRequirementsMod(mod.id)}>{text.requirements}{mod.nexus_source.requirements.items.length ? ` (${mod.nexus_source.requirements.items.length})` : ""}</button>}
              <button type="button" className="btn btn-secondary" aria-label={`${text.remove}: ${identity.name}`} title={text.remove} disabled={locked} onClick={() => void remove(mod.id)}><MenuIcon name="trash" /></button>
            </div>}
          </div>;
        })}</div>}
        {searchOpen && !visibleMods.length && <p className="nexus-hint" role="status">{text.noMatchingLocalMods}</p>}
        {game && installReview && <NexusInstallSequence game={game} modId={installReview} busy={busy} errorMessage={error} onInstall={onInstall} onToggle={onToggle} onClose={() => setInstallReview(null)} onError={onError} />}
        {game?.mods.filter(mod => mod.id === requirementsMod).map(mod => <NexusPopup key={mod.id} title={text.requirements} onClose={() => setRequirementsMod(null)}><h3 className="nexus-install-mod-name">{display(mod).name}</h3>{mod.nexus_source && <NexusRequirements game={game} requirements={mod.nexus_source.requirements} onOpen={url => void openUrl(url).catch(e => onError(String(e)))} />}{mod.nexus_source && <button type="button" className="btn btn-secondary" onClick={() => void openUrl(`https://www.nexusmods.com/${mod.nexus_source!.domain}/mods/${mod.nexus_source!.mod_id}`).catch(e => onError(String(e)))}>{text.modPage}</button>}{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>)}
        <div className="nexus-mod-tools">{game && <button type="button" className="btn btn-secondary" onClick={() => onLogs(game.id)}><MenuIcon name="logs" />{text.logs}</button>}
        {game?.mods.length ? <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void invoke("open_folder", { path: game.mods[0].archive.replace(/\/[^/]+$/, "") }).catch(e => onError(String(e)))}><MenuIcon name="folder" />{text.archives}</button> : null}<button type="button" className="btn btn-primary nexus-import-action" disabled={!game || busy || choosing} onClick={() => void importArchive()}><MenuIcon name="plusCircle" />{busy || choosing ? text.loading : text.localMod}</button></div>
      </>
    </motion.div></AnimatePresence>
      <section className="nexus-settings-panel menu-glass-panel" aria-label={text.settings}>
        <div className="nexus-settings-content" tabIndex={0}>
        <div className="nexus-settings-title"><h3 className="nexus-settings-heading"><MenuIcon name="settings" />{text.settings}</h3>{settingsSaved && <span className="nexus-settings-saved" role="status"><span className="nexus-saved-circle"><MenuIcon name="check" /></span>{text.settingsSaved}</span>}</div>
        {game && <>
          <div className="nexus-settings-fields game-info-fields">
            {[{ key: "directory", label: text.directory, value: directory, set: setDirectory, choose: text.choose }, ...(game.platform === "proton" ? [{ key: "prefix", label: text.prefix, value: compatData, set: setCompatData, choose: text.choosePrefix }] : [])].map(field => <div className="installation-field nexus-settings-path" key={field.key}>
              <label htmlFor={`nexus-settings-${field.key}`}>{field.label}</label>
              <HoverTooltip text={field.value} anchorClassName="installation-path-tooltip-anchor" tooltipClassName="installation-path-tooltip">
                <div className="game-directory-value installation-path-box">
                  <input id={`nexus-settings-${field.key}`} className="installation-path-input" value={field.value} disabled={settingsLocked || choosing} onChange={event => field.set(event.target.value)} onBlur={() => void saveSettings(field.key === "directory" ? {directory:field.value} : {prefix:field.value})} onKeyDown={event => { if(event.key === "Enter") event.currentTarget.blur(); }} />
                  <button type="button" className="installation-path-picker" disabled={settingsLocked || choosing} aria-label={field.choose} onClick={() => { setChoosing(true); void openDirectoryPicker(field.choose, field.value || directory).then(path => { if (path) { field.set(path); void saveSettings(field.key === "directory" ? {directory:path} : {prefix:path}); } }).catch(error => onError(String(error))).finally(() => setChoosing(false)); }}><MenuIcon name="folderSearch" /></button>
                </div>
              </HoverTooltip>
            </div>)}
            <NexusDeployOptions game={game} disabled={settingsLocked} onChange={method => onConfigureDeploy(game.id,method)} onSaved={showSaved} />
            {game.platform === "proton" && <ProtonSelector compactStyle label="Proton" value={proton} onChange={value => { setProton(value); void saveSettings({proton:value}); }} disabled={settingsLocked} />}
            <div className="game-info-item nexus-settings-platform">
              <span className="game-info-label"><MenuIcon name="platform" />{text.platform}:</span>
              <span className="info-value-pill info-platform">{game.platform === "proton" ? "Windows (Proton / Wine)" : text.native}</span>
            </div>
          </div>
        </>}
        <details className="nexus-module-details"><summary>{text.moduleLabel}</summary><NexusModuleStatus game={game} /></details>
        </div>
      </section>
    </div>
    {tab === "catalog" && <NexusCatalogPanel game={game} onAssociate={onAssociate} onImport={onImport} onClose={() => setTab("mods")} />}
  </div>;
}
