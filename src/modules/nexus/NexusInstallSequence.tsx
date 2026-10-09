import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { localInstallOrder } from "./installOrder";
import { NexusRequirements } from "./NexusRequirements";
import { openBrowserUrl } from "../../services/browser";
import { NexusPopup } from "./NexusPopup";
import { NexusInstallPlan, type Plan } from "./NexusInstallPlan";
import { nexusTarget, plainNexusText } from "./nexusLinks";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
import type { NexusGame, LocalMod } from "./useNexusWorkspace";

export function NexusInstallSequence({ game, modId, busy, errorMessage, onInstall, onToggle, onClose, onError }: {
  game: NexusGame; modId: string; busy: boolean; errorMessage?: string;
  onInstall: (gameId: string, modId: string, selection?: string[]) => Promise<boolean>;
  onToggle: (gameId: string, modId: string, enabled: boolean) => Promise<boolean>;
  onClose: () => void; onError: (error: string) => void;
}) {
  const text = useNexusText();
  const [order, setOrder] = useState<string[]>([]), [index, setIndex] = useState(0);
  const [loading, setLoading] = useState(true), [error, setError] = useState("");
  const [ready, setReady] = useState(false), [selection, setSelection] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);
  const lock = useRef(false);
  const domain = game.adapter || game.nexus_domain;
  // Rebuild when another archive arrives, not when a step becomes installed.
  const available = JSON.stringify(game.mods.map(mod => [mod.id, mod.nexus_source]));
  useEffect(() => {
    let active = true;
    setLoading(true); setReady(false); setError("");
    void (async () => {
      try { const ordered = await localInstallOrder(game, modId, id => invoke<Plan>("plan_nexus_installation", {gameId: game.id, modId: id, selection: null}), () => active, text.dependencyCycle); if (active) { setOrder(ordered); setIndex(0); } }
      catch (e) { if (active) setError(String(e)); }
      finally { if (active) setLoading(false); }
    })();
    return () => { active = false; };
  }, [game.id, modId, available, domain]);
  const current = game.mods.find(mod => mod.id === order[index]);
  const authored = current?.nexus_source?.requirements;
  const authoredReady = !authored?.items.some(item => {
    if (/\boptional\b|opcional/i.test(item.notes)) return false;
    const target = item.kind === "nexus" ? nexusTarget(item.url ?? null) : null;
    if (!target || target.domain !== domain) return false;
    return !game.mods.some(mod => mod.nexus_source?.domain === target.domain && mod.nexus_source.mod_id === target.id && mod.installed && mod.enabled)
      && !game.detected_mods?.some(mod => mod.mod_id === target.id && mod.enabled);
  });
  const apply = async () => {
    if (!current || busy || saving || lock.current || !authoredReady || (!current.installed && !ready)) return;
    lock.current = true; setSaving(true); setError("");
    try {
      const ok = current.installed ? current.enabled || await onToggle(game.id, current.id, true) : await onInstall(game.id, current.id, selection);
      if (ok) { if (index + 1 >= order.length) onClose(); else { setReady(false); setSelection([]); setIndex(value => value + 1); } }
    } catch (e) { setError(String(e)); onError(String(e)); }
    finally { lock.current = false; setSaving(false); }
  };
  const localReview = (local: LocalMod) => { const step = order.indexOf(local.id); if (step >= 0) { setReady(false); setSelection([]); setIndex(step); } };
  return <NexusPopup className="nexus-install-review-popup" title={text.reviewInstall} onClose={() => {if (!lock.current) onClose();}} footer={<button type="button" className="btn btn-primary" disabled={loading || busy || saving || !!error || !current || !authoredReady || (!current.installed && !ready)} onClick={() => void apply()}><MenuIcon name="download" />{saving ? text.installing : current?.installed && !current.enabled ? text.enable : text.install}{order.length > 1 && ` (${index + 1}/${order.length})`}</button>}>
    {loading && <p role="status" className="nexus-hint">{text.loading}</p>}
    {(error || errorMessage) && <p role="alert" className="nexus-error">{error || errorMessage}</p>}
    {!loading && current && <>
      {order.length > 1 && <ol className="nexus-install-order" aria-label={text.installOrder}>{order.map((id, step) => {const mod = game.mods.find(item => item.id === id); return <li key={id} className={step === index ? "current" : step < index ? "complete" : ""}><span>{step < index ? <MenuIcon name="check" /> : step + 1}</span><strong>{plainNexusText(mod?.name || id)}</strong><small>{step < index ? text.installed : step === index ? text.installFirst : step === order.length - 1 ? text.queuePrimary : text.queueDependency}</small></li>;})}</ol>}
      {order.length === 1 && <h3 className="nexus-install-mod-name">{plainNexusText(current.name)}</h3>}
      {!current.installed && <NexusInstallPlan key={current.id} game={game} modId={current.id} onReady={setReady} onSelection={setSelection} onInstallLocal={localReview} requirements={authored} />}
      {current.installed && authored && (authored.items.length > 0 || !authored.complete || authored.error) && <NexusRequirements game={game} requirements={authored} disabled={busy || saving} onInstallLocal={localReview} onOpen={url => void openBrowserUrl(url).catch(e => setError(String(e)))} />}

    </>}
  </NexusPopup>;
}
