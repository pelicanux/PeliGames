import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
interface View { entries: { name: string; managed: string | null }[]; ini: string }
/** Fallout 76 archive order (community Vortex extension "BA2 Archive Load Order"). */
export function NexusArchiveOrder({ game, disabled, onClose }: { game: NexusGame; disabled: boolean; onClose: () => void }) {
  const text = useNexusText();
  const [view, setView] = useState<View | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    void invoke<View>("nexus_archive_order", { gameId: game.id }).then(value => { if (active.current) setView(value); }).catch(e => { if (active.current) setError(String(e)); });
    return () => { active.current = false; };
  }, [game.id]);
  const move = (index: number, up: boolean) => {
    if (!view || working) return;
    const order = view.entries.map(entry => entry.name), target = index + (up ? -1 : 1);
    if (target < 0 || target >= order.length) return;
    [order[index], order[target]] = [order[target], order[index]];
    setWorking(true); setError("");
    void invoke<View>("set_nexus_archive_order", { gameId: game.id, order }).then(value => { if (active.current) setView(value); })
      .catch(e => { if (active.current) setError(String(e)); }).finally(() => { if (active.current) setWorking(false); });
  };
  const locked = disabled || working || !view;
  return <NexusPopup className="nexus-load-order-popup" title={text.archiveOrder} onClose={onClose}>
    <p className="nexus-hint">{text.archiveOrderHint}</p>
    {view && <div className="nexus-tabs"><button type="button" className="btn btn-secondary" onClick={() => void invoke("open_folder", { path: view.ini }).catch(e => setError(String(e)))}><MenuIcon name="logs" />{text.loadOrderOpenFile}</button></div>}
    {view && !view.entries.length && <p className="nexus-hint">{text.archiveOrderEmpty}</p>}
    {view && view.entries.length > 0 && <ol className="nexus-load-order-list">{view.entries.map((entry, index) => <li key={entry.name} className="nexus-mod-row nexus-load-order-row">
      <span className="nexus-load-order-index">{index + 1}</span>
      <span className="nexus-installed-mod-info"><span className="nexus-mod-title"><strong>{entry.name}</strong></span><small>{entry.managed ?? text.archiveOrderManual}</small></span>
      <span className="nexus-mod-order-actions">
        <button type="button" className="btn btn-secondary" aria-label={`${text.moveUp}: ${entry.name}`} title={text.moveUp} disabled={locked || index === 0} onClick={() => move(index, true)}><MenuIcon name="chevronUp" /></button>
        <button type="button" className="btn btn-secondary" aria-label={`${text.moveDown}: ${entry.name}`} title={text.moveDown} disabled={locked || index === view.entries.length - 1} onClick={() => move(index, false)}><MenuIcon name="chevronDown" /></button>
      </span>
    </li>)}</ol>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </NexusPopup>;
}
