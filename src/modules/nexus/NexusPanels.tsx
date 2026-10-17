import { ModOptionSwitch } from "../../components/ModOptionRow";
import { AdvancedGameSettings } from "../../components/AdvancedGameSettings";
import type { AdvancedSettings } from "../../services/advancedSettings";
import { NexusLaunchOptions } from "./NexusLaunchOptions";
import { NexusGameBackup } from "./NexusGameBackup";
import { NexusDeployOptions } from "./NexusDeployOptions";
import { NexusLootControl } from "./NexusLootControl";
import { NexusLoadOrder } from "./NexusLoadOrder";
import { NexusPlugins } from "./NexusPlugins";
import { NexusProfiles, type ProfileSummary } from "./NexusProfiles";
import { NexusSaves } from "./NexusSaves";
import { NexusArchiveOrder } from "./NexusArchiveOrder";
import { useNexusGameTools, type NexusGameTools } from "./useNexusGameTools";
import { NexusGameChecks } from "./NexusGameChecks";
import { nexusArchiveFormats } from "./archiveFormats";
import { useEffect, useId, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusPopup, type NexusPanelPreview } from "./NexusPopup";
import { NexusRequirements } from "./NexusRequirements";
import { ModernGameOptions } from "../../components/modern/ModernGameOptions";
import { HoverTooltip } from "../../components/HoverTooltip";
import { MenuIcon } from "../../components/MenuIcon";
import { openDirectoryPicker, openFilePicker } from "../../services/tauriService";
import { NexusInstallSequence } from "./NexusInstallSequence";
import { NexusDeployCounter } from "./NexusDeployProgress";
import { NexusModuleStatus } from "./NexusModuleStatus";
import { NexusCollectionDialog } from "./NexusCollectionDialog";
import { listen } from "@tauri-apps/api/event";
import { useNexusText } from "./text";
import type { NexusGame, LocalMod } from "./useNexusWorkspace";
import { modDisplay } from "./modDisplay";
/** Automatic loader downloads already started in this session (`gameId:name`). */
const automaticDownloads = new Set<string>();
export function NexusPanels({ section, toolState, tab, onTabChange, highlightedModId, highlightRequestId, game, busy, settingsBusy, error, reducedMotion, progress, onImport, onRemove, onInstall, onToggle, onMove, onSwitchProfile, onDownloadLoader, onLogs, onConfigure, onConfigureDeploy, onConfigureLaunch, onConfigureAdvanced, onUninstall, onError }: {
  section?: "mods" | "tools" | "performance" | "options" | "advanced";
  toolState?: NexusGameTools;
  onUninstall?: () => void;
  tab: "mods" | "catalog"; onTabChange: (tab: "mods" | "catalog") => void;
  highlightedModId?: string; highlightRequestId?: number;
  game?: NexusGame; busy: boolean; settingsBusy: boolean; error: string; reducedMotion: boolean; progress?: import("./useNexusWorkspace").DeployProgress;
  onImport: (gameId: string, source: string) => Promise<boolean>;
  onInstall: (gameId: string, modId: string, selection?: string[]) => Promise<boolean>;
  onToggle: (gameId: string, modId: string, enabled: boolean) => Promise<boolean>;
  onMove?: (gameId: string, modId: string, up: boolean) => Promise<boolean>;
  onSwitchProfile?: (gameId: string, id: string) => Promise<boolean>;
  onDownloadLoader?: (gameId: string, name?: string) => Promise<boolean>;
  onConfigure: (gameId: string, compatData: string, proton: string, directory: string) => Promise<boolean>;
  onConfigureDeploy: (gameId: string, method: import("./useNexusWorkspace").DeployMethod) => Promise<boolean>;
  onConfigureAdvanced: (gameId: string, settings: AdvancedSettings) => Promise<boolean>;
  onConfigureLaunch: (gameId: string, options: string | null) => Promise<boolean>;
  onLogs: (gameId: string) => void;
  onRemove: (gameId: string, modId: string) => Promise<boolean>; onError: (error: string) => void;
}) {
  const text = useNexusText();
  const [selectingMods, setSelectingMods] = useState(false);
  const [reordering, setReordering] = useState(false);
  const reorderLock = useRef(false);
  const draggedMod = useRef<{ gameId: string; id: string; x: number; y: number; active: boolean } | null>(null);
  const dragDestination = useRef<{ id: string; after: boolean } | null>(null);
  const [dropTarget, setDropTarget] = useState<{ id: string; after: boolean } | null>(null);
  useEffect(() => { setSelectingMods(false); }, [game?.id]);
  const modsPanel = useRef<HTMLDivElement>(null);
  const panelPreview = useRef<NexusPanelPreview | null>(null);
  const [expandedPanel, setExpandedPanel] = useState<DOMRect | null>(null);
  const [selectedMods, setSelectedMods] = useState<string[]>([]);
  const [removeSelection, setRemoveSelection] = useState<string[] | null>(null);
  const [selectionWorking, setSelectionWorking] = useState(false);
  const selectionLock = useRef(false);
  const [togglingMod, setTogglingMod] = useState<{ gameId: string; modId: string } | null>(null);
  const toggleMod = async (gameId: string, modId: string, enabled: boolean) => {
    setTogglingMod({ gameId, modId });
    try { return await onToggle(gameId, modId, enabled); }
    finally { setTogglingMod(current => current?.gameId === gameId && current.modId === modId ? null : current); }
  };

  useEffect(() => { setSelectedMods([]); setRemoveSelection(null); }, [game?.id, Boolean(expandedPanel)]);
  useEffect(() => {
    setSelectedMods(current => current.filter(id => game?.mods.some(mod => mod.id === id)));
  }, [game?.mods]);
  useEffect(() => { setExpandedPanel(null); }, [game?.id]);
  const [advanced, setAdvanced] = useState(section === "performance");
  const [dllRefresh, setDllRefresh] = useState(0);
  const [dlls, setDlls] = useState("");
  const [dllError, setDllError] = useState("");
  const dllRevision = game?.mods.map(mod => `${mod.id}:${mod.installed}:${mod.enabled}`).join("|");
  useEffect(() => {
    if ((!advanced && section !== "options" && section !== "advanced") || !game) return;
    let active = true; setDlls(""); setDllError("");
    void invoke<{dll_overrides:string}>("get_nexus_launch_options",{gameId:game.id}).then(value => { if(active) setDlls(value.dll_overrides); }).catch(error => {if(active) setDllError(String(error));});
    return () => {active=false;};
  },[advanced,section,game?.id,game?.launch_options,game?.advanced?.dll_overrides,dllRevision,dllRefresh]);
  useEffect(() => { setAdvanced(section === "performance"); }, [game?.id, section]);
  const [moduleDetailsGame, setModuleDetailsGame] = useState<string | null>(null);
  const [backupStatusText, setBackupStatusText] = useState("");
  useEffect(() => { setBackupStatusText(""); }, [game?.id]);
  useEffect(() => { setModuleDetailsGame(null); }, [game?.id]);
  // Each mounted panel owns its ref: closing the overlay must not clear the compact list's ref.
  const modList = useRef<HTMLDivElement>(null);
  const expandedModList = useRef<HTMLDivElement>(null);
  const searchInput = useRef<HTMLInputElement>(null);
  const searchId = useId();
  const [searchOpen, setSearchOpen] = useState(false);
  const [modQuery, setModQuery] = useState("");
  const [expandedModId, setExpandedModId] = useState<string | null>(null);
  const [labels, setLabels] = useState<Record<string, { name: string; version: string }>>({});
  const display = (mod: LocalMod) => labels[mod.id] || modDisplay(mod);
  const visibleMods = (game?.mods || []).filter(mod => display(mod).name.toLocaleLowerCase().includes(modQuery.trim().toLocaleLowerCase()));
  const modOrderRevision = `${game?.id}:${visibleMods.map(mod => mod.id).join("|")}`;
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
      const list = expandedModList.current;
      const row = Array.from(list?.children || []).find(row => (row as HTMLElement).dataset.modId === expandedModId);
      if (!row?.contains(event.target as Node)) setExpandedModId(null);
    };
    document.addEventListener("pointerdown", outside);
    return () => document.removeEventListener("pointerdown", outside);
  }, [expandedModId, expandedPanel]);
  useEffect(() => { setSearchOpen(false); setModQuery(""); }, [game?.id]);
  useEffect(() => { if (searchOpen) searchInput.current?.focus(); }, [searchOpen]);
  useEffect(() => { if (highlightedModId) { setModQuery(""); setSearchOpen(false); } }, [highlightedModId, highlightRequestId]);
  useEffect(() => { (expandedModList.current)?.scrollTo({top: 0}); }, [modQuery, expandedPanel]);
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
  // After the loader download, open the installation of the new archive.
  const [pendingInstall, setPendingInstall] = useState<Set<string> | null>(null);
  useEffect(() => {
    if (!pendingInstall || !game) return;
    const added = game.mods.find(mod => !pendingInstall.has(mod.id) && !mod.installed);
    if (added) { setPendingInstall(null); setInstallReview(added.id); }
  }, [game?.mods, pendingInstall]); // eslint-disable-line react-hooks/exhaustive-deps
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
  const localTools = useNexusGameTools(toolState ? undefined : game);
  const {tools, loadOrderKind, savesSupported, extras, setExtras, error: toolsError} = toolState ?? localTools;
  useEffect(() => { if (toolsError) onError(toolsError); }, [toolsError, onError]);
  const [loadOrderOpen, setLoadOrderOpen] = useState(false);
  const [pluginsOpen, setPluginsOpen] = useState(false);
  const [profilesOpen, setProfilesOpen] = useState(false);
  const [savesOpen, setSavesOpen] = useState(false);
  const [archivesOpen, setArchivesOpen] = useState(false);
  // Mod profiles of the game; the active one is named in the heading when there are several.
  const [profiles, setProfiles] = useState<ProfileSummary[]>([]);
  useEffect(() => {
    let active = true; setProfiles([]); setProfilesOpen(false);
    if (game) void invoke<ProfileSummary[]>("nexus_user_profiles", { gameId: game.id }).then(value => { if (active) setProfiles(value); }).catch(() => undefined);
    return () => { active = false; };
  }, [game?.id, dllRevision]);
  useEffect(() => { setLoadOrderOpen(false); setPluginsOpen(false); }, [game?.id]);
  const downloadLoader = async (target: NexusGame, name?: string) => {
    if (!onDownloadLoader) return false;
    const before = new Set(target.mods.map(mod => mod.id));
    const ok = await onDownloadLoader(target.id, name);
    if (ok) setPendingInstall(before);
    return ok;
  };
  // Loaders the extension downloads by itself (autoDownloadUe4ss): once per session,
  // only when missing and no downloaded package is waiting to be installed.
  useEffect(() => {
    if (!game || !extras || busy || game.running) return;
    const check = extras.checks.find(item => item.automatic && item.kind === "missing_download");
    if (!check || automaticDownloads.has(`${game.id}:${check.path}`)) return;
    if (game.mods.some(mod => !mod.installed && mod.name.toLowerCase().startsWith(check.path.toLowerCase()))) return;
    automaticDownloads.add(`${game.id}:${check.path}`);
    void downloadLoader(game, check.path);
  }, [extras, game?.id]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => { setSavesOpen(false); }, [game?.id]);
  const supported = Boolean(game?.adapter);
  const locked = busy || selectionWorking || reordering || Boolean(game?.running);
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
  const selected = (game?.mods || []).filter(mod => selectedMods.includes(mod.id));
  const runSelection = async (action: "enable" | "disable" | "remove") => {
    if (!game || locked || settingsBusy || selectionLock.current) return;
    const gameId = game.id;
    const targets = action === "remove" ? [...(removeSelection || [])] : selected
      .filter(mod => mod.installed && mod.enabled !== (action === "enable")).map(mod => mod.id);
    if (!targets.length) return;
    selectionLock.current = true; setSelectionWorking(true); onError("");
    try {
      let completed = true;
      // Await each deployment; never race modifications of the same game profile.
      for (const id of targets) {
        if (activeGame.current !== gameId) { completed = false; break; }
        const ok = action === "remove" ? await onRemove(gameId, id) : await toggleMod(gameId, id, action === "enable");
        if (!ok) { completed = false; break; }
        if (action === "remove") {
          setSelectedMods(current => current.filter(value => value !== id));
          setRemoveSelection(current => current?.filter(value => value !== id) || null);
        }
      }
      if (completed && activeGame.current === gameId) setRemoveSelection(null);
    } catch (e) { onError(String(e)); }
    finally { selectionLock.current = false; setSelectionWorking(false); }
  };
  const importArchive = async () => {
    if (!game || busy || choosing) return;
    setChoosing(true);
    try { const source = await openFilePicker("Mods", nexusArchiveFormats); if (source) await onImport(game.id, source); }
    catch (e) { onError(String(e)); } finally { setChoosing(false); }
  };
  const moveModTo = async (id: string, target: number) => {
    if (!game || !onMove || locked || settingsBusy || reorderLock.current) return;
    const source = game.mods.findIndex(mod => mod.id === id);
    if (source < 0 || source === target) return;
    const gameId = game.id, up = target < source;
    reorderLock.current = true; setReordering(true);
    try {
      for (let step = 0; step < Math.abs(target - source); step++) {
        if (activeGame.current !== gameId || !await onMove(gameId, id, up)) break;
      }
    } catch (error) { onError(String(error)); }
    finally { reorderLock.current = false; setReordering(false); }
  };
  const renderManagement = (large: boolean) => <>
        {large && game && extras && <NexusGameChecks game={game} extras={extras} onDownloadLoader={onDownloadLoader ? (name?: string) => downloadLoader(game, name) : undefined} disabled={locked || settingsBusy} onChange={setExtras} />}
        <div className="nexus-mod-tools">{section === "tools" && (savesSupported || ["bethesda", "baldurs_gate3", "archive_list"].includes(loadOrderKind)) && <h4>{text.loadOrder}</h4>}
        {null}{large && loadOrderKind === "baldurs_gate3" && <button type="button" className="btn btn-secondary" disabled={!game} onClick={() => setLoadOrderOpen(true)}><MenuIcon name="arrowUpDown" />{text.loadOrder}</button>}{large && loadOrderKind === "bethesda" && <button type="button" className="btn btn-secondary" disabled={!game} onClick={() => setPluginsOpen(true)}><MenuIcon name="puzzle" />{text.plugins}</button>}{large && loadOrderKind === "archive_list" && <button type="button" className="btn btn-secondary" disabled={!game} onClick={() => setArchivesOpen(true)}><MenuIcon name="arrowUpDown" />{text.archiveOrder}</button>}{large && savesSupported && <button type="button" className="btn btn-secondary" disabled={!game} onClick={() => setSavesOpen(true)}><MenuIcon name="floppy" />{text.saves}</button>}{section === "tools" && game && <NexusLootControl gameId={game.id} disabled={settingsLocked} onError={onError} onSaved={showSaved} />}</div>
        {large && tools.length > 0 && <section className="nexus-game-tools" aria-label={text.gameTools}><h4>{text.gameTools}</h4><div className="nexus-tabs">{tools.map(tool => <button type="button" className="btn btn-secondary" key={tool.name} disabled={!game || locked} onClick={() => { if (game) void invoke("open_nexus_game_tool", {gameId:game.id,name:tool.name}).catch(error => onError(String(error))); }}><MenuIcon name="settings" />{text.openManager}: {tool.name}</button>)}</div></section>}
  </>;
  const renderMods = (active: boolean, large: boolean) => game && !game.mods.length ? (
    <div className="nexus-modern-empty-mods">
      <h3>{text.noInstalledMods}</h3>
      <p>{text.emptyModsHint}</p>
      <div className="nexus-empty-choices">
        <div className="nexus-empty-choice"><span>{text.onlineCatalog}</span>
          <button type="button" className="nexus-empty-square nexus-empty-catalog" aria-label={text.installFromCatalog} disabled={locked} onClick={() => setTab("catalog")}>
            <span className="nexus-empty-puzzle-art" aria-hidden="true"><span className="nexus-install-piece"><MenuIcon name="puzzle" /></span></span>
          </button>
        </div>
        <div className="nexus-empty-choice"><span>{choosing ? text.loading : text.localMod}</span>
          <button type="button" className="nexus-empty-square nexus-empty-local" aria-label={text.installArchive} disabled={locked || choosing} onClick={() => void importArchive()}>
            <svg width="62" height="62" viewBox="0 0 48 48" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" aria-hidden="true"><circle cx="24" cy="24" r="20" /><path d="M24 14v20M14 24h20" /></svg>
          </button>
        </div>
      </div>
    </div>
  ) : (
      <><div className="nexus-panel-heading nexus-modern-manager-heading">
          <h3>{text.manageMods}{!installReview && !togglingMod && <NexusDeployCounter progress={progress} />}</h3>
          <div className="nexus-manager-shortcuts">
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={locked} onClick={() => setTab("catalog")}><span>{text.onlineCatalog}</span><span className="nexus-empty-puzzle-art" aria-hidden="true"><span className="nexus-install-piece"><MenuIcon name="puzzle" /></span></span></button>
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={locked || choosing} onClick={() => void importArchive()}><span>{choosing ? text.loading : text.localMod}</span><MenuIcon name="plusCircle" /></button>
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={locked || settingsBusy} onClick={() => setProfilesOpen(true)}><span>{text.profile}</span><MenuIcon name="sliders" /></button>
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={busy || !game?.mods.length} onClick={() => { if (game?.mods[0]) void invoke("open_folder", {path: game.mods[0].archive.replace(/\/[^/]+$/, "")}).catch(e => onError(String(e))); }}><span>{text.modFolder}</span><MenuIcon name="folder" /></button>
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={!game} onClick={() => { if (game) onLogs(game.id); }}><span>{text.logs}</span><MenuIcon name="logs" /></button>
            <button type="button" className="nexus-empty-square nexus-manager-square" disabled={locked} onClick={() => setCollectionReference("")}><span>{text.collection}</span><MenuIcon name="puzzle" /></button>
          </div>

        </div>

        {["dragonageinquisition", "starwarsbattlefront22017"].includes(game?.adapter || "") && <p className="nexus-hint">{text.managerRequired}</p>}
        {(searchOpen || (large)) && <label className="nexus-mod-search-field"><MenuIcon name="search" /><input ref={active ? searchInput : undefined} id={`${searchId}-${large ? "large" : "small"}`} type="search" aria-label={text.searchInstalledMods} placeholder={text.searchInstalledMods} value={modQuery} onChange={event => setModQuery(event.target.value)} onKeyDown={event => { if (event.key === "Escape") { event.stopPropagation(); setModQuery(""); setSearchOpen(false); } }} /></label>}

        {!game?.mods.length ? <div className="nexus-empty-mods nexus-install-start"><button type="button" className="nexus-install-puzzle" aria-label={text.installFromCatalog} disabled={!game || busy} onClick={() => setTab("catalog")}><span className="nexus-install-piece"><MenuIcon name="puzzle" /></span><span className="nexus-install-download"><MenuIcon name="download" /></span></button><span>{text.installFromCatalog}</span></div> : <motion.div layoutScroll ref={large ? expandedModList : modList} className="nexus-mod-list" data-scrollbar-outside="true" data-scrollbar-persistent={"true"}>{visibleMods.map(mod => {
          const identity = display(mod), expanded = false;
          const Summary = "div";
          const state = mod.installed ? mod.enabled ? "active" : "inactive" : "pending";
          return <motion.div layout={active && !reducedMotion ? "position" : false} layoutDependency={modOrderRevision} initial={false} transition={{ layout: { duration: .28, ease: [.4, 0, .2, 1] } }} className={`nexus-mod-row nexus-installed-mod-row nexus-mod-${state} ${expanded ? "expanded" : "collapsed"} ${activeHighlight === mod.id ? "nexus-mod-highlighted" : ""} ${selectingMods ? "nexus-mod-selecting" : ""} ${dropTarget?.id === mod.id ? dropTarget.after ? "nexus-drop-after" : "nexus-drop-before" : ""}`} data-mod-id={mod.id} ref={active && highlightedModId === mod.id ? highlightedRow : undefined} tabIndex={0} key={mod.id}
            data-reorderable={Boolean(onMove && !locked && !settingsBusy && !selectingMods)}
            aria-keyshortcuts={"Alt+ArrowUp Alt+ArrowDown"}
            onPointerDown={event => {
              if (!onMove || locked || settingsBusy || selectingMods || event.button !== 0 || (event.target as Element).closest("button, input, a, label")) return;
              draggedMod.current = {gameId: game.id, id: mod.id, x: event.clientX, y: event.clientY, active: false};
              dragDestination.current = null;
              event.currentTarget.setPointerCapture(event.pointerId);
              event.currentTarget.focus({preventScroll: true});
            }}
            onPointerMove={event => {
              const source = draggedMod.current;
              if (!source || source.gameId !== game.id || locked || settingsBusy) return;
              if (!source.active && Math.hypot(event.clientX - source.x, event.clientY - source.y) < 5) return;
              source.active = true; event.preventDefault();
              const list = expandedModList.current, viewport = list?.getBoundingClientRect();
              if (list && viewport) { if (event.clientY < viewport.top + 36) list.scrollTop -= 16; else if (event.clientY > viewport.bottom - 36) list.scrollTop += 16; }
              const target = document.elementFromPoint(event.clientX, event.clientY)?.closest<HTMLElement>("[data-mod-id]");
              if (!target || !list?.contains(target)) { dragDestination.current = null; setDropTarget(null); return; }
              const bounds = target.getBoundingClientRect();
              const destination = {id: target.dataset.modId!, after: event.clientY > bounds.top + bounds.height / 2};
              dragDestination.current = destination; setDropTarget(destination);
            }}
            onPointerUp={event => {
              const source = draggedMod.current, destination = dragDestination.current;
              draggedMod.current = null; dragDestination.current = null; setDropTarget(null);
              if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId);
              if (!source?.active || !destination || source.gameId !== game.id) return;
              const slot = game.mods.findIndex(item => item.id === destination.id) + (destination.after ? 1 : 0);
              const from = game.mods.findIndex(item => item.id === source.id);
              void moveModTo(source.id, Math.max(0, Math.min(game.mods.length - 1, slot - (from < slot ? 1 : 0))));
            }}
            onPointerCancel={() => { draggedMod.current = null; dragDestination.current = null; setDropTarget(null); }}
            onKeyDown={event => {
              if (!event.altKey || event.target !== event.currentTarget || !["ArrowUp", "ArrowDown"].includes(event.key)) return;
              event.preventDefault(); const index = game.mods.findIndex(item => item.id === mod.id);
              void moveModTo(mod.id, Math.max(0, Math.min(game.mods.length - 1, index + (event.key === "ArrowUp" ? -1 : 1))));
            }}>
            {selectingMods && <input className="nexus-mod-select" type="checkbox" aria-label={`${text.selectMod}: ${identity.name}`} checked={selectedMods.includes(mod.id)} disabled={locked || settingsBusy} onChange={event => setSelectedMods(current => event.target.checked ? [...current, mod.id] : current.filter(id => id !== mod.id))} />}
            
            <Summary className="nexus-mod-summary" aria-expanded={undefined} aria-controls={undefined} onClick={undefined}>
              <span className="nexus-mod-position">{game.mods.findIndex(item => item.id === mod.id) + 1}</span>
              <span className="nexus-installed-mod-info"><span className="nexus-mod-title"><strong>{identity.name}</strong><NexusDeployCounter progress={!installReview && togglingMod?.gameId === game.id && togglingMod.modId === mod.id ? progress : undefined} /></span><small>{identity.version ? `v${identity.version} · ` : ""}{mod.nexus_source ? "Nexus Mods · " : ""}{(mod.size / 1024 / 1024).toFixed(1)} MB</small></span>
              
              
            </Summary>
            <div className="nexus-mod-visible-toggle"><HoverTooltip text={text.requirements}><button type="button" className="nexus-mod-info-button" aria-label={`${text.requirements}: ${identity.name}`} onClick={() => setRequirementsMod(mod.id)}><MenuIcon name="info" /></button></HoverTooltip>{mod.installed ? <ModOptionSwitch label={`${mod.enabled ? text.disable : text.enable}: ${identity.name}`} enabled={mod.enabled} disabled={locked || settingsBusy || !supported || Boolean(togglingMod)} onToggle={() => void toggleMod(game.id, mod.id, !mod.enabled)} /> : <button type="button" className="btn btn-secondary nexus-mod-inline-install" disabled={locked || settingsBusy} onClick={() => setInstallReview(mod.id)} aria-label={`${text.install}: ${identity.name}`}><MenuIcon name="download" />{text.install}</button>}</div>
            {expanded && mod.identification_note && <small className="nexus-local-identification-note" role="status">{mod.identification_note}</small>}
            {expanded && extras?.extender_errors.filter(item => item.mod_id === mod.id).map(item => <small key={item.dll} className="nexus-local-identification-note nexus-error" role="status">{text.extenderErrors}: {item.dll} · {item.message}</small>)}
            {expanded && mod.installed && mod.enabled && (() => {
              const wins = overrides.filter(item => item.winner === mod.id).length, loses = overrides.filter(item => item.overridden.includes(mod.id)).length;
              return (wins || loses) ? <small className="nexus-local-identification-note" role="status">{[wins ? `${text.overridesOthers}: ${wins}` : "", loses ? `${text.overriddenBy}: ${loses}` : ""].filter(Boolean).join(" · ")}</small> : null;
            })()}
            {expanded && <div id={`mod-actions-${large ? "large" : "small"}-${mod.id}`} className="nexus-installed-mod-actions">
              
              <button type="button" className="btn btn-secondary nexus-requirements-link" onClick={() => setRequirementsMod(mod.id)}>{text.requirements}{mod.nexus_source?.requirements.items.length ? ` (${mod.nexus_source.requirements.items.length})` : ""}</button>
              <div className="nexus-mod-order-actions">
              {onMove && <><button type="button" className="btn btn-secondary" aria-label={`${text.moveUp}: ${identity.name}`} title={text.moveUp} disabled={locked || game.mods[0]?.id === mod.id} onClick={() => void onMove(game.id, mod.id, true)}><MenuIcon name="chevronUp" /></button>
              <button type="button" className="btn btn-secondary" aria-label={`${text.moveDown}: ${identity.name}`} title={text.moveDown} disabled={locked || game.mods[game.mods.length - 1]?.id === mod.id} onClick={() => void onMove(game.id, mod.id, false)}><MenuIcon name="chevronDown" /></button></>}
              <button type="button" className="btn btn-secondary" aria-label={`${text.remove}: ${identity.name}`} title={text.remove} disabled={locked} onClick={() => void remove(mod.id)}><MenuIcon name="trash" /></button>
              </div>
            </div>}
          </motion.div>;
        })}</motion.div>}
        {searchOpen && !visibleMods.length && <p className="nexus-hint" role="status">{text.noMatchingLocalMods}</p>}
        {Boolean(game?.mods.length) && <div className="nexus-mod-batch-bar" aria-label={text.batchActions}>
          <span className="nexus-batch-hint" role="status">{reordering ? text.reorderingMods : selected.length ? `${selected.length} ${selected.length === 1 ? text.selectedMod : text.selectedMods}` : text.batchHint}</span>
          <button type="button" className="btn btn-secondary" aria-pressed={selectingMods} disabled={locked || settingsBusy} onClick={() => { setSelectingMods(value => !value); setSelectedMods([]); }}>{selectingMods ? text.cancel : text.selectMultiple}</button>
          {selectingMods && <button type="button" className="btn btn-secondary" disabled={locked || settingsBusy} onClick={() => setSelectedMods(visibleMods.map(mod => mod.id))}>{text.selectAllMods}</button>}
          {selected.length > 0 && <>
            <HoverTooltip text={text.enableSelected}><button type="button" className="btn btn-secondary" aria-label={text.enableSelected} disabled={locked || settingsBusy || !supported || !selected.some(mod => mod.installed && !mod.enabled)} onClick={() => void runSelection("enable")}><MenuIcon name="chainLinked" /></button></HoverTooltip>
            <HoverTooltip text={text.disableSelected}><button type="button" className="btn btn-secondary" aria-label={text.disableSelected} disabled={locked || settingsBusy || !supported || !selected.some(mod => mod.installed && mod.enabled)} onClick={() => void runSelection("disable")}><MenuIcon name="chainBroken" /></button></HoverTooltip>
            <HoverTooltip text={text.removeSelected}><button type="button" className="btn btn-secondary" aria-label={text.removeSelected} disabled={locked || settingsBusy} onClick={() => { onError(""); setRemoveSelection([...selectedMods]); }}><MenuIcon name="trash" /></button></HoverTooltip>
          </>}
        </div>}
        
      </>
  );
  const standardSettings = game && <>          <div className="nexus-settings-fields game-info-fields">
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
            
            <NexusGameBackup key={game.id} gameId={game.id} disabled={settingsLocked} onError={onError} onSaved={showSaved} onStatusTextChange={undefined} />
            
            
          </div></>;
  if (section === "advanced" && game) return <div className="modern-advanced-tab"><AdvancedGameSettings hideExecutionEditors embedded value={game.advanced} disabled={settingsLocked} reducedMotion={reducedMotion} saved={settingsSaved} automaticDllOverrides={dlls} dllOverridesError={dllError} onResetDlls={()=>setDllRefresh(value=>value+1)} onChange={value=>{void onConfigureAdvanced(game.id,value).then(ok=>{if(ok) showSaved();});}} isSteam={game.game.launcher === "Steam"} appId={game.game.app_id} /></div>;
  if (section === "options" && game) return <ModernGameOptions key={game.id} name={game.game.name} value={game.advanced} proton={proton} native={game.game.native_linux || game.platform === "native"} disabled={settingsLocked} maintenancePath={`nexus:${game.id}`} onUninstall={onUninstall}
    onProton={value => onConfigure(game.id, game.compat_data || "", value, game.game.directory || game.game.path)} onAdvanced={value => onConfigureAdvanced(game.id,value)} automaticDlls={dlls} dllError={dllError} onResetDlls={() => setDllRefresh(value=>value+1)}
    configuration={<div className="modern-options-section modern-nexus-settings"><h4>{text.settings}</h4>{standardSettings}</div>}
    launchOptions={<NexusLaunchOptions inline gameId={game.id} disabled={settingsLocked} busy={settingsBusy} errorMessage={error} onSave={onConfigureLaunch}/>} />;
  return <div className={`nexus-panel nexus-panel-modern`}>
    {expandedPanel && <NexusPopup className="nexus-expanded-mods-popup menu-glass-panel" title={text.title} zoomFrom={expandedPanel} zoomPreview={() => panelPreview.current} reducedMotion={reducedMotion} onBack={() => setExpandedPanel(null)} onClose={() => setExpandedPanel(null)}><div className="nexus-mods-content nexus-expanded-mods-content">{renderMods(true, true)}</div></NexusPopup>}
        {game && profilesOpen && <NexusProfiles game={game} profiles={profiles} disabled={locked || settingsBusy} onChange={setProfiles} onSwitch={async id => { const ok = onSwitchProfile ? await onSwitchProfile(game.id, id) : false; if (ok) setProfiles(await invoke<ProfileSummary[]>("nexus_user_profiles", { gameId: game.id })); return ok; }} onClose={() => setProfilesOpen(false)} />}
        {game && archivesOpen && <NexusArchiveOrder game={game} disabled={locked || settingsBusy} onClose={() => setArchivesOpen(false)} />}
        {game && savesOpen && <NexusSaves game={game} disabled={locked || settingsBusy} onClose={() => setSavesOpen(false)} />}
        {game && pluginsOpen && <NexusPlugins game={game} disabled={locked || settingsBusy} onClose={() => setPluginsOpen(false)} />}
        {game && loadOrderOpen && <NexusLoadOrder game={game} disabled={locked || settingsBusy} onClose={() => setLoadOrderOpen(false)} />}
        {game && collectionReference !== null && <NexusCollectionDialog game={game} initialReference={collectionReference || undefined} onClose={() => setCollectionReference(null)} onQueued={() => undefined} />}
        {game && installReview && <NexusInstallSequence game={game} modId={installReview} busy={busy} progress={progress} errorMessage={error} onInstall={onInstall} onToggle={onToggle} onClose={() => setInstallReview(null)} onError={onError} />}
        {game?.mods.filter(mod => mod.id === requirementsMod).map(mod => <NexusPopup key={mod.id} title={text.requirements} onClose={() => setRequirementsMod(null)}><h3 className="nexus-install-mod-name">{display(mod).name}</h3>{mod.identification_note && <p className="nexus-hint">{mod.identification_note}</p>}{extras?.extender_errors.filter(item => item.mod_id === mod.id).map(item => <p key={item.dll} className="nexus-error">{text.extenderErrors}: {item.dll} · {item.message}</p>)}{!mod.nexus_source && <p className="nexus-hint">{text.requirementsUnavailable}</p>}{mod.nexus_source && <NexusRequirements game={game} requirements={mod.nexus_source.requirements} onOpen={url => void openUrl(url).catch(e => onError(String(e)))} />}{mod.nexus_source && <><p className="nexus-mod-usage-hint">{text.modUsageHint}</p><button type="button" className="btn btn-secondary" onClick={() => void openUrl(`https://www.nexusmods.com/${mod.nexus_source!.domain}/mods/${mod.nexus_source!.mod_id}`).catch(e => onError(String(e)))}>{text.modPage}</button></>}{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>)}

    {game && removeSelection && removeSelection.length > 0 && <NexusPopup className="nexus-remove-popup" title={text.removeSelected} onClose={() => { if (!selectionLock.current) setRemoveSelection(null); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={selectionWorking} onClick={() => setRemoveSelection(null)}>{text.cancel}</button>
      <button type="button" className="btn btn-primary" disabled={locked || settingsBusy} onClick={() => void runSelection("remove")}><MenuIcon name="trash" />{selectionWorking ? text.loading : text.remove}</button>
    </>}><p className="nexus-hint">{text.removeSelectedPrompt}</p><ul>{game.mods.filter(mod => removeSelection.includes(mod.id)).map(mod => <li key={mod.id}>{display(mod).name}</li>)}</ul>{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>}
    {modToRemove && <NexusPopup className="nexus-remove-popup" title={text.remove} onClose={() => { if (!removalLock.current) setRemoveTarget(null); }} footer={<>
      <button type="button" className="btn btn-secondary" disabled={removing} onClick={() => setRemoveTarget(null)}>{text.cancel}</button>
      <button type="button" className="btn btn-primary" disabled={locked || settingsBusy || removing} onClick={() => void confirmRemoval()}><MenuIcon name="trash" />{removing ? text.loading : text.remove}</button>
    </>}><strong className="nexus-remove-name">{display(modToRemove).name}</strong><p className="nexus-hint">{text.removePrompt}</p>{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>}
    <div className={"nexus-mod-panels"}>
      {(!section || section === "mods") && (<div ref={modsPanel} role="region" aria-label={text.mods} className="nexus-tab-content nexus-mods-content nexus-modern-mods-layout nexus-modern-workspace">{renderMods(true, true)}</div>)}
      {section === "tools" && <div className="nexus-modern-game-tools">{renderManagement(true)}{error && <p role="alert" className="nexus-error">{error}</p>}</div>}
      {section !== "mods" && section !== "tools" && <section className="nexus-settings-panel menu-glass-panel" aria-label={text.settings}>
        <AnimatePresence mode="wait" initial={false}><motion.div key={advanced ? "advanced" : "standard"} className="nexus-settings-content" data-scrollbar-outside="true" tabIndex={0}
          initial={{x: reducedMotion ? 0 : "100%"}} animate={{x:0}} exit={{x:reducedMotion ? 0 : "-100%"}} transition={{duration:reducedMotion ? 0 : .29,ease:[.4,0,.2,1]}}>
        {advanced && game ? <AdvancedGameSettings key={game.id} embedded onResetDlls={() => setDllRefresh(value => value + 1)} automaticDllOverrides={dlls} dllOverridesError={dllError} value={game.advanced} disabled={settingsLocked} reducedMotion={reducedMotion} saved={settingsSaved} onBack={section === "performance" ? undefined : () => setAdvanced(false)} onChange={value => { void onConfigureAdvanced(game.id,value).then(ok => {if(ok) showSaved();}); }} isSteam={game.game.launcher === "Steam"} appId={game.game.app_id} launchOptions={<NexusLaunchOptions gameId={game.id} disabled={settingsLocked} busy={settingsBusy} errorMessage={error} onSave={onConfigureLaunch} />} /> : <>
        <div className="nexus-settings-title"><h3 className="nexus-settings-heading"><MenuIcon name="settings" />{text.settings}</h3>{settingsSaved && <span className="nexus-settings-saved" role="status"><span className="nexus-saved-circle"><MenuIcon name="check" /></span>{text.settingsSaved}</span>}</div>
        {game && <>
          {standardSettings}
        </>}
        <div className="nexus-settings-info-links">
          {game && <HoverTooltip text={backupStatusText} tooltipClassName="nexus-backup-status-tooltip"><button type="button" className="nexus-module-link nexus-backup-info-link"><span>{text.backupLatest}</span></button></HoverTooltip>}
          <button type="button" className="nexus-module-link" aria-haspopup="dialog" disabled={!game} onClick={() => setModuleDetailsGame(game?.id || null)}><span>{text.moduleLabel}</span></button>
        </div>
        </>}
        </motion.div></AnimatePresence>
      </section>}
    </div>
    
    {game && moduleDetailsGame === game.id && <NexusPopup className="nexus-module-popup" title={text.moduleLabel} onClose={() => setModuleDetailsGame(null)} reducedMotion={reducedMotion}>
      <NexusModuleStatus game={game} />
    </NexusPopup>}
  </div>;
}
