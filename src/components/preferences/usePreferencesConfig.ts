import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../SetupWizard";

export function usePreferencesConfig(onUpdated: (config: AppConfig | null) => void) {
  const [config, setConfig] = useState<AppConfig>({ backend: "AMDNR", dll_version: "0.5.1", shortcut_key: "Insert" });
  const [ready, setReady] = useState(false), [saving, setSaving] = useState(false), [error, setError] = useState("");
  const latest = useRef(config), persisted = useRef(config), revision = useRef(0), pending = useRef(0), mounted = useRef(true);
  const chain = useRef(Promise.resolve()), callback = useRef(onUpdated);
  callback.current = onUpdated;
  const load = async () => {
    setError("");
    try {
      const stored = await invoke<AppConfig | null>("load_app_config");
      if (mounted.current) { if (stored) { latest.current = stored; persisted.current = stored; setConfig(stored); } setReady(true); }
    } catch (failure) { if (mounted.current) setError(String(failure)); }
  };
  useEffect(() => { mounted.current = true; void load(); return () => { mounted.current = false; }; }, []);
  const save = (patch: Partial<AppConfig>) => {
    if (!ready) return;
    const value = { ...latest.current, ...patch };
    if (JSON.stringify(value) === JSON.stringify(latest.current) && !error) return;
    latest.current = value; setConfig(value); setError(""); setSaving(true);
    const currentRevision = ++revision.current; ++pending.current;
    // Serialize complete snapshots so concurrent field changes cannot overwrite each other.
    chain.current = chain.current.then(async () => {
      try {
        await invoke("save_app_config", { config: value });
        const previous = persisted.current;
        if (mounted.current && currentRevision === revision.current) {
          persisted.current = value;
          callback.current(value);
          if (previous.steamgriddb_api_key !== value.steamgriddb_api_key) window.dispatchEvent(new Event("steamGridSettingsChanged"));
          if (previous.scan_steam_protons !== value.scan_steam_protons) window.dispatchEvent(new Event("protonRunnersChanged"));
        }
      } catch (failure) { if (mounted.current && currentRevision === revision.current) setError(String(failure)); }
      finally { --pending.current; if (mounted.current && !pending.current) setSaving(false); }
    });
  };
  return { config, ready, saving, error, save, retry: () => ready ? save({}) : void load(), isPending: () => pending.current > 0 };
}
