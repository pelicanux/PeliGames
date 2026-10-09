import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { NexusPopup } from "./NexusPopup";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";

/** The remote native webview occupies only this popup's content rectangle. */
export function NexusBrowserPopup() {
  const text = useNexusText();
  const [label, setLabel] = useState<string | null>(null);
  const [error, setError] = useState("");
  const viewport = useRef<HTMLDivElement>(null);
  useEffect(() => {
    let active = true, changed = false;
    const update = (next: string | null) => { changed = true; if (active) { setLabel(next); setError(""); } };
    const preview = (event: Event) => update((event as CustomEvent<string | null>).detail);
    window.addEventListener("nexus-browser-preview", preview);
    const subscription = listen<string | null>("nexus-browser-session", event => update(event.payload));
    void subscription.then(() => invoke<string | null>("nexus_browser_session")).then(next => { if (active && !changed) setLabel(typeof next === "string" ? next : null); }).catch(() => {});
    return () => { active = false; window.removeEventListener("nexus-browser-preview", preview); void subscription.then(unlisten => unlisten()).catch(() => {}); };
  }, []);
  useEffect(() => {
    const element = viewport.current;
    if (!label || !element || label === "preview") return;
    let active = true, frame = 0, previous = "", syncing = false, dirty = false;
    const sync = async () => {
      if (!active) return;
      if (syncing) { dirty = true; return; }
      const rect = element.getBoundingClientRect();
      const bounds = {label, x: rect.x, y: rect.y, width: rect.width, height: rect.height};
      const key = JSON.stringify(bounds);
      if (rect.width < 1 || rect.height < 1 || key === previous) return;
      syncing = true;
      try { await invoke("resize_nexus_browser", bounds); if (active) { previous = key; setError(""); } }
      catch (e) { if (active) setError(String(e)); }
      finally { syncing = false; if (active && dirty) { dirty = false; schedule(); } }
    };
    const schedule = () => { cancelAnimationFrame(frame); frame = requestAnimationFrame(() => void sync()); };
    const observer = new ResizeObserver(schedule);
    observer.observe(element); observer.observe(document.documentElement);
    window.addEventListener("resize", schedule);
    schedule();
    return () => { active = false; cancelAnimationFrame(frame); observer.disconnect(); window.removeEventListener("resize", schedule); };
  }, [label]);
  const close = () => {
    if (!label) return;
    void invoke("close_nexus_browser", {label}).then(() => setLabel(current => current === label ? null : current)).catch(e => setError(String(e)));
  };
  const action = (action: string) => { if (label) void invoke("nexus_browser_action", {label, action}).catch(e => setError(String(e))); };
  if (!label) return null;
  return <NexusPopup title={text.nexusBrowserTitle} className="nexus-browser-popup" onClose={close}
    headerActions={<div className="nexus-browser-controls"><button type="button" className="btn btn-secondary" onClick={() => action("reload")}><MenuIcon name="repair" />{text.browserReload}</button><button type="button" className="btn btn-secondary" onClick={() => action("external")}><MenuIcon name="platform" />{text.externalBrowser}</button></div>}>
    <div className="nexus-browser-content">
      <div className="nexus-browser-origin">nexusmods.com</div>
      {error && <p role="alert" className="nexus-error">{error}</p>}
      <div className="nexus-browser-viewport" ref={viewport}>
        {label === "preview" && <p className="nexus-browser-placeholder">{text.browserPreview}</p>}
      </div>
    </div>
  </NexusPopup>;
}
