import { AdvancedGameSettings } from "../../components/AdvancedGameSettings";
import type { AdvancedSettings } from "../../services/advancedSettings";
import { NexusLaunchOptions } from "./NexusLaunchOptions";
import { NexusGameBackup } from "./NexusGameBackup";
import { NexusDeployOptions } from "./NexusDeployOptions";
import { NexusLootControl } from "./NexusLootControl";
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
import { NexusCollectionDialog } from "./NexusCollectionDialog";
import { listen } from "@tauri-apps/api/event";
import { useNexusText } from "./text";
import type { NexusGame, LocalMod } from "./useNexusWorkspace";
import { modDisplay } from "./modDisplay";
export function NexusPanels({ tab, onTabChange, highlightedModId, highlightRequestId, game, busy, settingsBusy, error, reducedMotion, onImport, onRemove, onInstall, onToggle, onMove, onLogs, onConfigure, onConfigureDeploy, onConfigureLaunch, onConfigureAdvanced, onAssociate, onError }: {
  tab: "mods" | "catalog"; onTabChange: (tab: "mods" | "catalog") => void;
  highlightedModId?: string; highlightRequestId?: number;
  game?: NexusGame; busy: boolean; settingsBusy: boolean; error: string; reducedMotion: boolean;
  onImport: (gameId: string, source: string) => Promise<boolean>;
  onInstall: (gameId: string, modId: string, selection?: string[]) => Promise<boolean>;
  onToggle: (gameId: string, modId: string, enabled: boolean) => Promise<boolean>;
  onMove?: (gameId: string, modId: string, up: boolean) => Promise<boolean>;
  onConfigure: (gameId: string, compatData: string, proton: string, directory: string) => Promise<boolean>;
  onConfigureDeploy: (gameId: string, method: import("./useNexusWorkspace").DeployMethod) => Promise<boolean>;
  onConfigureAdvanced: (gameId: string, settings: AdvancedSettings) => Promise<boolean>;
  onConfigureLaunch: (gameId: string, options: string | null) => Promise<boolean>;
  onAssociate: (gameId: string, domain: string) => Promise<boolean>;
  onLogs: (gameId: string) => void;
  onRemove: (gameId: string, modId: string) => Promise<boolean>; onError: (error: string) => void;
}) {
  const text = useNexusText();
  const [advanced, setAdvanced] = useState(false);
  const [dllRefresh, setDllRefresh] = useState(0);
  const [dlls, setDlls] = useState("");
  const [dllError, setDllError] = useState("");
  const dllRevision = game?.mods.map(mod => `${mod.id}:${mod.installed}:${mod.enabled}`).join("|");
  useEffect(() => {
    if (!advanced || !game) return;
    let active = true; setDlls(""); setDllError("");
    void invoke<{dll_overrides:string}>("get_nexus_launch_options",{gameId:game.id}).then(value => { if(active) setDlls(value.dll_overrides); }).catch(error => {if(active) setDllError(String(error));});
    return () => {active=false;};
  },[advanced,game?.id,game?.launch_options,game?.advanced?.dll_overrides,dllRevision,dllRefresh]);
  useEffect(() => { setAdvanced(false); }, [game?.id]);
  const [moduleDetailsGame, setModuleDetailsGame] = useState<string | null>(null);
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
  // Files replaced by mod priority in the deployed profile, refreshed when the list changes.
  const [overrides, setOverrides] = useState<{ target: string; winner: string; overridden: string[] }[]>([]);
  const overrideSignature = JSON.stringify([game?.id, game?.profile, game?.mods.map(mod => [mod.id, mod.installed, mod.enabled])]);
  useEffect(() => {
    let active = true;
    if (!game) { setOverrides([]); return; }
    void invoke<{ target: string; winner: string; overridden: string[] }[]>("nexus_mod_overrides", { gameId: game.id })
      .then(value => { if (active) setOverrides(value); }).catch(() => { if (active) setOverrides([]); });
    return () => { active = false; };
  }, [overrideSignature]); // eslint-disable-line react-hooks/exhaustive-deps
  // null: closed; "": open without link; otherwise a collection link (e.g. from nxm://).
  const [collectionReference, setCollectionReference] = useState<string | null>(null);
  useEffect(() => {
    let active = true, stop: (() => void) | undefined;
    void listen<string>("nexus-collection-request", event => {
      // Only a link for this game opens here; other games keep their own page.
      const domain = /^nxm:\/\/([a-z0-9]+)\//.exec(event.payload)?.[1];
      if (active && game && domain === game.nexus_domain) setCollectionReference(event.payload);
    }).then(unlisten => { if (active) stop = unlisten; else unlisten(); }).catch(() => undefined);
    return () => { active = false; stop?.(); };
  }, [game]);
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
      <><div className="nexus-panel-heading"><h3><img className="nexus-icon" src="/nexus-mods.svg" alt="" />{text.title}</h3>{Boolean(game?.mods.length) && (needsModSearch || searchOpen) && <HoverTooltip text={text.searchInstalledMods} anchorClassName="nexus-mod-search-trigger"><button type="button" className="nexus-mod-search-icon" aria-label={text.searchInstalledMods} aria-expanded={searchOpen} aria-controls={searchId} onClick={() => { setSearchOpen(value => !value); setModQuery(""); }}><MenuIcon name="search" /></button></HoverTooltip>}<button type="button" className="btn btn-primary" disabled={!game || busy || choosing} onClick={() => void importArchive()}><MenuIcon name="plusCircle" />{choosing ? text.loading : text.localMod}</button></div>
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
            {expanded && mod.installed && mod.enabled && (() => {
              const wins = overrides.filter(item => item.winner === mod.id).length, loses = overrides.filter(item => item.overridden.includes(mod.id)).length;
              return (wins || loses) ? <small className="nexus-local-identification-note" role="status">{[wins ? `${text.overridesOthers}: ${wins}` : "", loses ? `${text.overriddenBy}: ${loses}` : ""].filter(Boolean).join(" · ")}</small> : null;
            })()}
            {expanded && <div id={`mod-actions-${mod.id}`} className="nexus-installed-mod-actions">
              <button type="button" className="btn btn-secondary" disabled={locked || (mod.installed && !supported)} onClick={() => { if (mod.installed) void onToggle(game.id, mod.id, !mod.enabled).then(ok => { if (ok) setExpandedModId(current => current === mod.id ? null : current); }); else setInstallReview(mod.id); }}><MenuIcon name={mod.installed ? "check" : "download"} />{mod.installed ? mod.enabled ? text.disable : text.enable : text.install}</button>
              {mod.nexus_source && <button type="button" className="btn btn-secondary nexus-requirements-link" onClick={() => setRequirementsMod(mod.id)}>{text.requirements}{mod.nexus_source.requirements.items.length ? ` (${mod.nexus_source.requirements.items.length})` : ""}</button>}
              <div className="nexus-mod-order-actions">
              {onMove && <><button type="button" className="btn btn-secondary" aria-label={`${text.moveUp}: ${identity.name}`} title={text.moveUp} disabled={locked || game.mods[0]?.id === mod.id} onClick={() => void onMove(game.id, mod.id, true)}><MenuIcon name="chevronUp" /></button>
              <button type="button" className="btn btn-secondary" aria-label={`${text.moveDown}: ${identity.name}`} title={text.moveDown} disabled={locked || game.mods[game.mods.length - 1]?.id === mod.id} onClick={() => void onMove(game.id, mod.id, false)}><MenuIcon name="chevronDown" /></button></>}
              <button type="button" className="btn btn-secondary" aria-label={`${text.remove}: ${identity.name}`} title={text.remove} disabled={locked} onClick={() => void remove(mod.id)}><MenuIcon name="trash" /></button>
              </div>
            </div>}
          </div>;
        })}</div>}
        {searchOpen && !visibleMods.length && <p className="nexus-hint" role="status">{text.noMatchingLocalMods}</p>}
        {game && collectionReference !== null && <NexusCollectionDialog game={game} initialReference={collectionReference || undefined} onClose={() => setCollectionReference(null)} onQueued={() => undefined} />}
        {game && installReview && <NexusInstallSequence game={game} modId={installReview} busy={busy} errorMessage={error} onInstall={onInstall} onToggle={onToggle} onClose={() => setInstallReview(null)} onError={onError} />}
        {game?.mods.filter(mod => mod.id === requirementsMod).map(mod => <NexusPopup key={mod.id} title={text.requirements} onClose={() => setRequirementsMod(null)}><h3 className="nexus-install-mod-name">{display(mod).name}</h3>{mod.nexus_source && <NexusRequirements game={game} requirements={mod.nexus_source.requirements} onOpen={url => void openUrl(url).catch(e => onError(String(e)))} />}{mod.nexus_source && <button type="button" className="btn btn-secondary" onClick={() => void openUrl(`https://www.nexusmods.com/${mod.nexus_source!.domain}/mods/${mod.nexus_source!.mod_id}`).catch(e => onError(String(e)))}>{text.modPage}</button>}{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>)}
        <div className="nexus-mod-tools">{game && <button type="button" className="btn btn-secondary" onClick={() => onLogs(game.id)}><MenuIcon name="logs" />{text.logs}</button>}
        {game?.mods.length ? <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void invoke("open_folder", { path: game.mods[0].archive.replace(/\/[^/]+$/, "") }).catch(e => onError(String(e)))}><MenuIcon name="folder" />{text.archives}</button> : null}<button type="button" className="btn btn-secondary" disabled={!game || busy} onClick={() => setCollectionReference("")}><MenuIcon name="puzzle" />{text.collection}</button></div>
      </>
    </motion.div></AnimatePresence>
      <section className="nexus-settings-panel menu-glass-panel" aria-label={text.settings}>
        <AnimatePresence mode="wait" initial={false}><motion.div key={`${game?.id}-${advanced ? "advanced" : "standard"}`} className="nexus-settings-content" tabIndex={0}
          initial={{x: reducedMotion ? 0 : "100%", opacity: reducedMotion ? 1 : 0}} animate={{x:0,opacity:1}} exit={{x:reducedMotion ? 0 : "-100%",opacity:reducedMotion ? 1 : 0}} transition={{duration:reducedMotion ? 0 : .29,ease:[.4,0,.2,1]}}>
        {advanced && game ? <AdvancedGameSettings embedded onResetDlls={() => setDllRefresh(value => value + 1)} automaticDllOverrides={dlls} dllOverridesError={dllError} value={game.advanced} disabled={settingsLocked} reducedMotion={reducedMotion} saved={settingsSaved} onBack={() => setAdvanced(false)} onChange={value => { void onConfigureAdvanced(game.id,value).then(ok => {if(ok) showSaved();}); }} launchOptions={<NexusLaunchOptions gameId={game.id} disabled={settingsLocked} busy={settingsBusy} errorMessage={error} onSave={onConfigureLaunch} />} /> : <>
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
            <NexusGameBackup gameId={game.id} disabled={settingsLocked} onError={onError} onSaved={showSaved} />
            <div className="nexus-launch-options-control installation-field"><label><MenuIcon name="settings" />{text.settings}</label><button type="button" className="btn btn-primary" disabled={settingsLocked} onClick={() => setAdvanced(true)}><MenuIcon name="sliders" />{text.advanced}</button></div>
            <NexusLootControl gameId={game.id} disabled={settingsLocked} onError={onError} onSaved={showSaved} />
          </div>
        </>}
        <details key={game?.id} className="nexus-module-details" onToggle={event => setModuleDetailsGame(event.currentTarget.open ? game?.id || null : null)}><summary>{text.moduleLabel}</summary>{game && moduleDetailsGame === game.id && <NexusModuleStatus game={game} />}</details>
        </>}
        </motion.div></AnimatePresence>
      </section>
    </div>
    {tab === "catalog" && <NexusCatalogPanel game={game} onAssociate={onAssociate} onImport={onImport} onClose={() => setTab("mods")} />}
  </div>;
}
