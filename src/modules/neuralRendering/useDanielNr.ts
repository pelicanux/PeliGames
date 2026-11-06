import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { GameInfo } from "../../components/GameGrid";
import { useI18n } from "../../i18n/I18nContext";
import type { AdvancedSettings } from "../../services/advancedSettings";
import { updatePeliGamesSettings } from "../../services/installedLibrary";
import { openFilePicker } from "../../services/tauriService";
import { notifyError } from "../../services/notifications";
import { DANIEL_COMPONENT_CHANGED, danielGame, loadDaniel, operateDaniel, withDanielWrapper, type DanielState } from "./danielNr";

export interface DanielPanelProps {
  game: GameInfo; disabled: boolean; running?: boolean;
  onSaved: (game: GameInfo) => void; onPersist?: (settings: AdvancedSettings) => Promise<void>;
  onBusy: (busy: boolean) => void;
}
type Action = "install" | "remove" | "toggle" | "link" | "switch-proxy" | "switch-version" | "recover-version";

export function useDanielNr({ game, disabled, running = false, onSaved, onPersist, onBusy }: DanielPanelProps) {
  const {t: tx} = useI18n();

  const [state, setState] = useState<DanielState>();
  const [executable, setExecutable] = useState("");
  const [dx12, setDx12] = useState(false);
  const [runtime, setRuntime] = useState("");
  const [proxy, setProxy] = useState("auto");
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);
  const [progress, setProgress] = useState("");
  const [error, setError] = useState("");
  const [diagnostics, setDiagnostics] = useState<string>();
  const mounted = useRef(true), operation = useRef(false), generation = useRef(0);
  const request = danielGame(game, executable);
  const unsupported = Boolean(game.native_linux || game.flatpak);
  const supportedProfile = Boolean(onPersist || game.external_id || game.launcher === "PeliGames");
  const blocked = disabled || running || busy || loading;
  const linked = Boolean(state && game.advanced?.wrappers?.some(w => w.program === state.wrapper && w.enabled));
  const selectedRuntime = runtime || state?.version || "0.6.0";
  const packageReady = Boolean(state?.runtimes.find(r => r.version === selectedRuntime)?.ready);

  useEffect(() => { mounted.current = true; return () => { mounted.current = false; generation.current++; }; }, []);
  const reload = async (clearError = true) => {
    const current = ++generation.current;
    setLoading(true);
    if (clearError) setError("");
    try {
      const next = await loadDaniel(request);
      if (mounted.current && current === generation.current) {
        setState(next);
        setRuntime(old => next.installed ? next.version : old || next.version);
        setProxy(old => next.installed ? next.loading_dll.current || "auto" : old);
      }
    } catch (e) { if (mounted.current && current === generation.current) setError(String(e)); }
    finally { if (mounted.current && current === generation.current) setLoading(false); }
  };
  useEffect(() => {
    setState(undefined); setProxy("auto"); setRuntime(""); setDx12(false); setProgress(""); setDiagnostics(undefined);
    if (unsupported) setLoading(false); else void reload();
  }, [game.path, executable, unsupported]);
  useEffect(() => {
    const refresh = () => { if (!operation.current && !unsupported) void reload(false); };
    window.addEventListener(DANIEL_COMPONENT_CHANGED, refresh);
    return () => window.removeEventListener(DANIEL_COMPONENT_CHANGED, refresh);
  }, [game.path, executable, unsupported]);
  useEffect(() => {
    let stopped = false;
    const subscription = listen<{ directory: string; message: string }>("danielNrProgress", event => {
      if (!stopped && operation.current && event.payload.directory === request.directory) setProgress(event.payload.message);
    });
    void subscription.catch(() => {});
    return () => { stopped = true; void subscription.then(unlisten => unlisten()).catch(() => {}); };
  }, [request.directory]);

  const persist = async (next: AdvancedSettings) => {
    const updated = onPersist ? await onPersist(next).then(() => ({ ...game, advanced: next }))
      : game.external_id ? await invoke<GameInfo>("configure_external_game", { settings: { external_id: game.external_id, advanced: next } })
        : await updatePeliGamesSettings({ path: game.path, name: game.name, executable: game.executable || game.path, proton: game.proton || "", prefix: game.prefix, advanced: next });
    onSaved({ ...game, ...updated, library_view: game.library_view });
    window.dispatchEvent(new Event("peligamesLibraryChanged"));
  };
  const fail = (e: unknown) => { if (mounted.current) setError(String(e)); notifyError("Daniel NR", String(e)); };
  const execute = async (action: Action, selection?: { runtime?: string; proxy?: string }) => {
    if (operation.current || !state || blocked) return;
    operation.current = true; setBusy(true); onBusy(true); setError(""); setProgress(tx("useDanielNr", "applying")); setDiagnostics(undefined);
    try {
      if (action === "link") {
        await persist(withDanielWrapper(game.advanced, state.wrapper, true));
      } else {
        if (action === "remove") await persist(withDanielWrapper(game.advanced, state.wrapper, false, true));
        const next = await operateDaniel(request, action === "toggle" ? (state.enabled ? "disable" : "enable") : action,
          "", dx12, selection?.proxy ?? proxy, selection?.runtime ?? selectedRuntime);
        if (mounted.current) {
          setState(next);
          setRuntime(next.version);
          setProxy(next.loading_dll.current || "auto");
        }
        if (action === "install" || action === "toggle") await persist(withDanielWrapper(game.advanced, next.wrapper, true));
      }
      if (mounted.current) setProgress(tx("useDanielNr", "saved_changes_apply_on_the_next_game_launch"));
    } catch (e) { if (mounted.current) setProgress(""); fail(e); }
    finally { operation.current = false; if (mounted.current) { setBusy(false); void reload(false); } onBusy(false); }
  };
  const selectRuntime = (value: string) => {
    if (blocked || state?.pending_version_change) return;
    if (!state?.installed) setRuntime(value);
    else if (value !== state.version && state.runtimes.some(r => r.version === value && r.ready)) void execute("switch-version", { runtime: value });
  };
  const selectProxy = (value: string) => {
    if (blocked || state?.pending_version_change) return;
    if (!state?.installed) setProxy(value);
    else if (value !== state.loading_dll.current) void execute("switch-proxy", { proxy: value });
  };
  const chooseExecutable = async () => {
    try {
      const path = await openFilePicker(tx("useDanielNr", "game_rendering_executable"), ["exe"], request.directory);
      if (path && mounted.current) setExecutable(path);
    } catch (e) { fail(e); }
  };
  const diagnose = () => { void invoke<string>("daniel_nr_diagnostics", { game: request }).then(value => { if (mounted.current) setDiagnostics(value); }).catch(fail); };
  return { state, request, dx12, setDx12, proxy, selectedRuntime, packageReady, busy, loading, progress, error, diagnostics,
    unsupported, supportedProfile, blocked, linked, reload, fail, execute, selectRuntime, selectProxy, chooseExecutable, diagnose };
}
