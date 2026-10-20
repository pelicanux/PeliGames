import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { createLatestWriteQueue } from "../services/latestWriteQueue";

type Settings = { enabled: boolean; renodx: boolean };
type Feedback = "" | "nextLaunch";
const KEY = "neural_startup_preferences";
function readPreferences(): Record<string, boolean> {
  try { return JSON.parse(localStorage.getItem(KEY) || "{}"); } catch { return {}; }
}
let migration: Promise<void> | undefined;
function migratePreferences(): Promise<void> {
  if (!migration) {
    const preferences = readPreferences();
    migration = Object.keys(preferences).length
      ? invoke<void>("migrate_neural_preferences", { preferences }).then(() => { localStorage.removeItem(KEY); }).catch(error => { migration = undefined; throw error; })
      : Promise.resolve();
  }
  return migration;
}
export function useNeuralStartup(path: string | undefined, installed: boolean) {
  const identity = JSON.stringify([path, installed]);
  const initial = path ? readPreferences()[path] ?? true : true;
  const drafts = useRef(new Map<string, Settings>());
  const intended = useRef<Settings>({ enabled: initial, renodx: false });
  const queueIdentity = useRef("");
  const queue = useRef<ReturnType<typeof createLatestWriteQueue<Settings, Feedback>> | null>(null);
  const [state, setState] = useState({ identity, enabled: initial, renodx: false, busy: installed, initializing: Boolean(path), error: "", feedback: "" as Feedback });
  useEffect(() => {
    let active = true;
    let confirmed: Settings = path ? drafts.current.get(path) ?? { enabled: initial, renodx: false } : { enabled: initial, renodx: false };
    intended.current = confirmed;
    setState({ identity, ...confirmed, busy: installed, initializing: Boolean(path), error: "", feedback: "" as Feedback });
    const writer = createLatestWriteQueue<Settings, Feedback>(async value => {
      const feedback: Feedback = installed ? "nextLaunch" : "";
      if (path && installed) {
        await migratePreferences();
        await invoke<void>("set_neural_startup", { gameDir: path, ...value });
      }
      confirmed = value;
      if (path) drafts.current.set(path, value);
      return feedback;
    }, (value, feedback, error) => {
      if (active) {
        intended.current = error === undefined ? value : confirmed;
        setState({ identity, ...intended.current, busy: false, initializing: false, error: error === undefined ? "" : String(error), feedback: error === undefined ? feedback ?? "" : "" });
      }
    });
    queue.current = writer;
    queueIdentity.current = identity;
    migratePreferences().then(async () => {
      const preferences = await invoke<Record<string, boolean>>("load_neural_preferences");
      return path && installed
        ? invoke<Settings>("get_neural_settings", { gameDir: path })
        : path ? drafts.current.get(path) ?? { enabled: preferences[path] ?? true, renodx: false } : { enabled: true, renodx: false };
    }).then(settings => {
      if (active) {
        confirmed = settings;
        intended.current = settings;
        if (path) drafts.current.set(path, settings);
        setState({ identity, ...settings, busy: false, initializing: false, error: "", feedback: "" as Feedback });
      }
    }).catch(error => { if (active) setState({ identity, enabled: initial, renodx: false, busy: false, initializing: false, error: String(error), feedback: "" as Feedback }); });
    return () => { active = false; writer.dispose(); if (queue.current === writer) queue.current = null; };
  }, [path, installed]);
  useEffect(() => {
    if (!state.feedback || state.identity !== identity) return;
    const timer = window.setTimeout(() => {
      setState(current => current === state ? { ...current, feedback: "" } : current);
    }, 3000);
    return () => window.clearTimeout(timer);
  }, [state, identity]);
  const enabled = state.identity === identity ? state.enabled : initial;
  const busy = state.identity === identity ? state.busy : installed;
  const initializing = state.identity === identity ? state.initializing : Boolean(path);
  const update = (patch: Partial<Settings>) => {
    if (!path || initializing || !queue.current || queueIdentity.current !== identity) return;
    const value = { ...intended.current, ...patch };
    intended.current = value;
    // Show intent immediately. Saving never dims or locks the switch.
    setState({ identity, ...value, busy: true, initializing: false, error: "", feedback: "" as Feedback });
    queue.current.enqueue(value);
  };
  return { enabled, renodx: state.identity === identity ? state.renodx : false, busy, initializing, feedback: state.identity === identity ? state.feedback : "", error: state.identity === identity ? state.error : "", change: (enabled: boolean) => update({ enabled }), changeRenodx: (renodx: boolean) => update({ renodx }) };
}
