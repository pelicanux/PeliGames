import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
interface Save { file: string; character: string; level: number; location: string; playtime: string; number: number; created: number; plugins: string[] }
interface Entry extends Save { missing: string[] }
interface View { dir: string; own: boolean; saves: Entry[]; errors: string[]; others: { id: string | null; name: string; count: number }[] }
/** Save games page of the Bethesda games (Vortex gamebryo-savegame-management). */
export function NexusSaves({ game, disabled, onClose }: { game: NexusGame; disabled: boolean; onClose: () => void }) {
  const text = useNexusText();
  const [view, setView] = useState<View | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [shots, setShots] = useState<Record<string, string>>({});
  const [removing, setRemoving] = useState<string | null>(null);
  const [transfer, setTransfer] = useState<{ from: string | null; saves: Save[]; chosen: string[]; keep: boolean } | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    void invoke<View>("nexus_saves", { gameId: game.id }).then(value => { if (active.current) setView(value); }).catch(e => { if (active.current) setError(String(e)); });
    return () => { active.current = false; };
  }, [game.id]);
  useEffect(() => {
    if (!open || shots[open] !== undefined) return;
    void invoke<string>("nexus_save_screenshot", { gameId: game.id, file: open }).then(url => { if (active.current) setShots(current => ({ ...current, [open]: url })); }).catch(() => undefined);
  }, [open]); // eslint-disable-line react-hooks/exhaustive-deps
  const run = (command: string, args: Record<string, unknown>, after?: () => void) => {
    if (working) return;
    setWorking(true); setError("");
    void invoke<View>(command, { gameId: game.id, ...args }).then(value => { if (active.current) { setView(value); after?.(); } })
      .catch(e => { if (active.current) setError(String(e)); }).finally(() => { if (active.current) setWorking(false); });
  };
  const locked = disabled || working || !view;
  const date = (seconds: number) => seconds ? new Date(seconds * 1000).toLocaleString() : "—";
  const chooseSource = (from: string | null) => {
    setError("");
    void invoke<Save[]>("nexus_other_saves", { gameId: game.id, from }).then(saves => { if (active.current) setTransfer({ from, saves, chosen: [], keep: true }); }).catch(e => setError(String(e)));
  };
  return <NexusPopup className="nexus-load-order-popup nexus-saves-popup" title={text.saves} onClose={onClose}>
    <p className="nexus-hint">{text.savesHint}{view && ` ${view.own ? text.savesOwn : text.savesShared}`}</p>
    {view && <div className="nexus-tabs">
      <button type="button" className="btn btn-secondary" onClick={() => void invoke("open_folder", { path: view.dir }).catch(e => setError(String(e)))}><MenuIcon name="folder" />{text.savesFolder}</button>
      {view.others.map(other => <button key={other.id ?? "shared"} type="button" className="btn btn-secondary" disabled={locked || !other.count} onClick={() => chooseSource(other.id)}><MenuIcon name="download" />{text.savesTransferFrom} {other.name} ({other.count})</button>)}
    </div>}
    {transfer && <section className="nexus-game-checks">
      <h4>{text.savesTransfer}</h4>
      <div className="nexus-mod-list">{transfer.saves.map(save => <label key={save.file} className="nexus-fomod-option"><input type="checkbox" checked={transfer.chosen.includes(save.file)} onChange={event => setTransfer({ ...transfer, chosen: event.target.checked ? [...transfer.chosen, save.file] : transfer.chosen.filter(file => file !== save.file) })} /><span><strong>{save.character}</strong> · {text.savesLevel} {save.level} · {save.location} · {date(save.created)}<small> · {save.file}</small></span></label>)}</div>
      <label className="nexus-fomod-option"><input type="checkbox" checked={transfer.keep} onChange={event => setTransfer({ ...transfer, keep: event.target.checked })} />{text.savesKeepSource}</label>
      <div className="nexus-tabs">
        <button type="button" className="btn btn-primary" disabled={locked || !transfer.chosen.length} onClick={() => run("transfer_nexus_saves", { from: transfer.from, files: transfer.chosen, keep: transfer.keep }, () => setTransfer(null))}><MenuIcon name="check" />{transfer.keep ? text.savesCopy : text.savesMove}</button>
        <button type="button" className="btn btn-secondary" onClick={() => setTransfer(null)}>{text.cancel}</button>
      </div>
    </section>}
    {view && !view.saves.length && <p className="nexus-hint">{text.savesEmpty}</p>}
    {view && view.saves.length > 0 && <ol className="nexus-load-order-list">{view.saves.map(save => <li key={save.file} className="nexus-mod-row nexus-load-order-row nexus-save-row">
      <span className="nexus-installed-mod-info">
        <button type="button" className="nexus-plugin-name" aria-expanded={open === save.file} onClick={() => setOpen(current => current === save.file ? null : save.file)}><strong>{save.character || save.file}</strong>{save.missing.length > 0 && <span className="nexus-error"> ⚠ {save.missing.length}</span>}</button>
        <small>{text.savesLevel} {save.level} · {save.location} · {save.playtime} · {date(save.created)} · #{save.number}</small>
        {save.missing.length > 0 && <small className="nexus-error">{text.savesMissing}: {save.missing.join(", ")}</small>}
        {open === save.file && <>
          {shots[save.file] && <img className="nexus-save-shot" src={shots[save.file]} alt={text.savesScreenshot} />}
          <small>{save.file}</small>
          <small>{text.plugins}: {save.plugins.join(", ")}</small>
          <span className="nexus-plugin-rule">
            <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => run("restore_nexus_save_plugins", { file: save.file })}><MenuIcon name="repair" />{text.savesRestorePlugins}</button>
            <button type="button" className="btn btn-secondary" disabled={locked} onClick={() => { if (removing === save.file) run("delete_nexus_saves", { files: [save.file] }, () => setRemoving(null)); else setRemoving(save.file); }}><MenuIcon name="trash" />{removing === save.file ? text.savesConfirmDelete : text.remove}</button>
          </span>
        </>}
      </span>
    </li>)}</ol>}
    {view?.errors.map(item => <p key={item} className="nexus-hint nexus-error">{item}</p>)}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </NexusPopup>;
}
