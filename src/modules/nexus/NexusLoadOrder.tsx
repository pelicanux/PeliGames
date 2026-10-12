import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { openFilePicker } from "../../services/tauriService";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
interface Entry { file: string; name: string; uuid: string | null; locked: boolean; managed: string | null; missing: string[]; late: string[] }
interface View { entries: Entry[]; auto_export: boolean; format: "v6" | "v7" | "v8"; settings: string; mod_fixer: boolean }
/** Baldur's Gate 3 load order page, matching the Vortex extension's actions. */
export function NexusLoadOrder({ game, disabled, onClose }: { game: NexusGame; disabled: boolean; onClose: () => void }) {
  const text = useNexusText();
  const [view, setView] = useState<View | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    void invoke<View>("nexus_load_order", { gameId: game.id }).then(value => { if (active.current) setView(value); }).catch(e => { if (active.current) setError(String(e)); });
    return () => { active.current = false; };
  }, [game.id]);
  const run = async (action: () => Promise<View | null>, done = "") => {
    if (working) return;
    setWorking(true); setError(""); setNotice("");
    try { const value = await action(); if (value && active.current) { setView(value); setNotice(done); } }
    catch (e) { if (active.current) setError(String(e)); }
    finally { if (active.current) setWorking(false); }
  };
  const locked = disabled || working || !view;
  const movable = view?.entries.filter(entry => !entry.locked) || [];
  const move = (file: string, up: boolean) => void run(async () => {
    const order = view!.entries.map(entry => entry.file), index = order.indexOf(file), target = index + (up ? -1 : 1);
    if (target < 0 || target >= order.length || view!.entries[target].locked) return null;
    [order[index], order[target]] = [order[target], order[index]];
    return invoke<View>("set_nexus_load_order", { gameId: game.id, order });
  });
  const importFrom = (kind: "game" | "lsx" | "json") => void run(async () => {
    const source = kind === "game" ? "" : await openFilePicker(kind === "lsx" ? "BG3 Load Order" : "BG3MM Load Order", [kind]);
    return source === null ? null : invoke<View>("import_nexus_load_order", { gameId: game.id, source });
  }, text.loadOrderImported);
  const exportTo = (toFile: boolean) => void run(async () => {
    const target = toFile ? await save({ defaultPath: "modsettings.lsx", filters: [{ name: "BG3 Load Order", extensions: ["lsx"] }] }) : "";
    return target === null ? null : invoke<View>("export_nexus_load_order", { gameId: game.id, target });
  }, text.loadOrderExported);
  return <NexusPopup className="nexus-load-order-popup" title={text.loadOrder} onClose={onClose}>
    <p className="nexus-hint">{text.loadOrderHint}</p>
    <div className="nexus-tabs">
      <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => importFrom("game")}><MenuIcon name="download" />{text.loadOrderImportGame}</button>
      <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => importFrom("lsx")}><MenuIcon name="folder" />{text.loadOrderImportFile}</button>
      <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => importFrom("json")}><MenuIcon name="folder" />{text.loadOrderImportBg3mm}</button>
      <button type="button" className="btn btn-primary" disabled={locked} onClick={() => exportTo(false)}><MenuIcon name="check" />{text.loadOrderExportGame}</button>
      <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => exportTo(true)}><MenuIcon name="folder" />{text.loadOrderExportFile}</button>
      <button type="button" className="btn btn-secondary" disabled={!view} onClick={() => { if (view) void invoke("open_folder", { path: view.settings }).catch(e => setError(String(e))); }}><MenuIcon name="logs" />{text.loadOrderOpenFile}</button>
    </div>
    {view && <div className="nexus-load-order-auto">
      <label className="nexus-fomod-option"><input type="checkbox" checked={view.auto_export} disabled={working} onChange={event => { const enabled = event.target.checked; void run(() => invoke<View>("set_nexus_load_order_auto_export", { gameId: game.id, enabled })); }} />{text.loadOrderAutoExport}</label>
      <HoverTooltip text={text.loadOrderAutoExportHint}><button type="button" className="installation-type-info" aria-label={text.loadOrderAutoExportHint}><MenuIcon name="info" /></button></HoverTooltip>
    </div>}
    {view?.mod_fixer && <p className="nexus-hint" role="status">{text.loadOrderModFixer} <button type="button" className="btn btn-secondary" onClick={() => void openUrl("https://www.nexusmods.com/baldursgate3/mods/141?tab=description").catch(e => setError(String(e)))}>{text.modFixerPage}</button></p>}
    {view && !view.entries.length && <p className="nexus-hint">{text.loadOrderEmpty}</p>}
    {view && view.entries.length > 0 && <ol className="nexus-load-order-list">{view.entries.map(entry => {
      const position = movable.indexOf(entry);
      return <li key={entry.file} className={`nexus-mod-row nexus-load-order-row ${entry.locked ? "locked" : ""}`}>
        <span className="nexus-load-order-index">{entry.locked ? <MenuIcon name="cube" /> : position + 1}</span>
        <span className="nexus-installed-mod-info"><span className="nexus-mod-title"><strong>{entry.name}</strong></span>
          <small>{entry.file}{entry.locked ? ` · ${text.loadOrderLocked}` : ""}{entry.managed ? ` · ${entry.managed}` : ` · ${text.loadOrderExternal}`}</small>
          {entry.missing.length > 0 && <small className="nexus-error">{text.loadOrderMissing}: {entry.missing.join(", ")}</small>}
          {entry.late.length > 0 && <small className="nexus-error">{text.loadOrderLate}: {entry.late.join(", ")}</small>}
        </span>
        {!entry.locked && <span className="nexus-mod-order-actions">
          <button type="button" className="btn btn-secondary" aria-label={`${text.moveUp}: ${entry.name}`} title={text.moveUp} disabled={locked || position === 0} onClick={() => move(entry.file, true)}><MenuIcon name="chevronUp" /></button>
          <button type="button" className="btn btn-secondary" aria-label={`${text.moveDown}: ${entry.name}`} title={text.moveDown} disabled={locked || position === movable.length - 1} onClick={() => move(entry.file, false)}><MenuIcon name="chevronDown" /></button>
        </span>}
      </li>;
    })}</ol>}
    {notice && <p className="nexus-hint" role="status">{notice}</p>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </NexusPopup>;
}
