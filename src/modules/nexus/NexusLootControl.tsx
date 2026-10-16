import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { useNexusText } from "./text";

interface LootStatus { supported: boolean; repository: string; updated?: number | null }
/** LOOT sorting is opt-in: plugins are sorted at deployment once the masterlist is downloaded. */
export function NexusLootControl({ gameId, disabled, onError, onSaved }: { gameId: string; disabled: boolean; onError: (error: string) => void; onSaved: () => void }) {
  const text = useNexusText();
  const [status, setStatus] = useState<LootStatus | null>(null), [busy, setBusy] = useState(false);
  const currentGame = useRef(gameId); currentGame.current = gameId;
  useEffect(() => {
    let active = true; setStatus(null);
    void invoke<LootStatus>("nexus_loot_status", { gameId }).then(value => { if (active) setStatus(value); }).catch(() => undefined);
    return () => { active = false; };
  }, [gameId]);
  if (!status?.supported) return null;
  const update = () => {
    const id = gameId; setBusy(true);
    void invoke<LootStatus>("update_nexus_loot_masterlist", { gameId: id })
      .then(value => { if (currentGame.current === id) { setStatus(value); onSaved(); } })
      .catch(error => { if (currentGame.current === id) onError(String(error)); })
      .finally(() => setBusy(false));
  };
  const date = status.updated ? new Date(status.updated * 1000).toLocaleDateString() : "";
  return <div className="installation-field nexus-loot-control">
    <label><MenuIcon name="document" />LOOT</label>
    <HoverTooltip text={text.lootHint} align="left" anchorClassName="nexus-loot-tooltip-anchor" tooltipClassName="modern-tools-tooltip">
      <button type="button" className="btn btn-secondary" disabled={disabled || busy} onClick={update}>
        <MenuIcon name="download" />{busy ? text.loading : status.updated ? `${text.lootUpdate} · ${date}` : text.lootEnable}
      </button>
    </HoverTooltip>
  </div>;
}
