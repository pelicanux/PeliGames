import { plainNexusText, safeLink, nexusTarget } from "./nexusLinks";
export { plainNexusText } from "./nexusLinks";
import { NexusInstallSequence } from "./NexusInstallSequence";
import { HoverTooltip } from "../../components/HoverTooltip";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusDownloadQueue } from "./NexusDownloadQueue";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { LocalMod, NexusGame } from "./useNexusWorkspace";
import { useNexusText } from "./text";
export interface Requirements {
  items: { name: string; notes: string; kind: "nexus" | "external" | "dlc"; url?: string | null }[];
  complete: boolean;
  error?: string | null;
}
interface DependencyDetails { info: { name: string }; files: { file_id: number; name: string; file_name: string; description?: string; version: string; size: number; category_name?: string; category_id?: number }[]; requirements: Requirements }
export function NexusRequirements({ requirements, onOpen, disabled = false, game, onInstallLocal }: {
  requirements?: Requirements | null; onOpen: (url: string) => void; disabled?: boolean; game?: NexusGame; onInstallLocal?: (mod: LocalMod) => void;
}) {
  const text = useNexusText();
  const [localSelection, setLocalSelection] = useState<string | null>(null);
  const [selection, setSelection] = useState<{ domain: string; id: number; details: DependencyDetails } | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const lock = useRef(false), generation = useRef(0);
  const signature = JSON.stringify(requirements?.items.map(item => item.url));
  useEffect(() => { generation.current++; lock.current = false; setSelection(null); setLocalSelection(null); setBusy(false); setError(""); setMessage(""); return () => { generation.current++; }; }, [game?.id, signature]);
  const choose = async (target: { domain: string; id: number }) => {
    if (lock.current || disabled) return;
    const request = generation.current; lock.current = true; setBusy(true); setError(""); setMessage("");
    try { const details = await invoke<DependencyDetails>("get_nexus_catalog_mod", { gameDomain: target.domain, modId: target.id }); if (request === generation.current) setSelection({ ...target, details }); }
    catch (e) { if (request === generation.current) setError(String(e)); }
    finally { if (request === generation.current) { lock.current = false; setBusy(false); } }
  };
  const enable = async (modId: string) => {
    if (!game || lock.current || disabled || game.running) return;
    const request = generation.current; lock.current = true; setBusy(true); setError("");
    try { await invoke("set_nexus_mod_enabled", { gameId: game.id, modId, enabled: true }); }
    catch (e) { if (request === generation.current) setError(String(e)); }
    finally { if (request === generation.current) { lock.current = false; setBusy(false); } }
  };
  const gameDomain = game?.adapter || game?.nexus_domain;
  return <section className="nexus-requirements">
    <h4>{text.requirements}</h4>
    {(!requirements?.complete || requirements.error) && <p className="nexus-requirements-warning" role="status">{text.requirementsUnknown}</p>}
    {requirements?.complete && !requirements.error && !requirements.items.length && <p className="nexus-hint">{text.requirementsNone}</p>}
    {!!requirements?.items.length && <><div className="nexus-requirement-list">{requirements.items.map((item, index) => {
      const url = safeLink(item.url), target = item.kind === "nexus" ? nexusTarget(url) : null;
      const matches = target ? game?.mods.filter(mod => mod.nexus_source?.domain === target.domain && mod.nexus_source.mod_id === target.id) : undefined;
      const registered = matches?.find(mod => mod.installed && mod.enabled) || matches?.find(mod => mod.installed) || matches?.[0];
      const normalize = (name: string) => name.toLowerCase().replace(/[^a-z0-9]/g, "");
      const requiredName = normalize(item.name);
      const detected = game?.detected_mods?.find(mod => {
        if (target && mod.mod_id === target.id && gameDomain === target.domain) return true;
        const name = normalize(mod.name);
        if (game.adapter === "palworld" && name === "ue4ss") return ["ue4ss", "reue4ss"].includes(requiredName);
        if (target?.domain === "palworld" && target.id === 577) return ["modconfigmenu", "dekmodconfigmenu", "dekmcm"].includes(name);
        return name === requiredName;
      });
      return <article key={`${item.kind}-${index}`}><div><strong>{plainNexusText(item.name)}</strong><small>{item.kind === "nexus" ? text.nexusRequirement : item.kind === "external" ? text.externalRequirement : text.dlcRequirement}</small>{detected && <span className="nexus-status">{detected.enabled ? text.dependencyDetected : text.dependencyDetectedDisabled}</span>}{!detected && registered && <span className="nexus-status">{!registered.installed ? text.dependencyDownloaded : registered.enabled ? text.dependencyInstalled : text.dependencyDisabled}</span>}{item.notes && <HoverTooltip text={plainNexusText(item.notes)}><button type="button" className="nexus-requirement-info" aria-label={plainNexusText(item.notes)}><MenuIcon name="info" /></button></HoverTooltip>}{target && gameDomain !== target.domain && <p className="nexus-hint">{text.dependencyOtherGame}</p>}</div><div className="nexus-requirement-actions">{url && <button type="button" className="btn btn-secondary" disabled={disabled || busy} onClick={() => onOpen(url)}>{item.kind === "external" && !detected ? text.externalDownload : text.viewMod}</button>}{target && <button type="button" className="btn btn-primary" disabled={disabled || busy || !game || game.running || gameDomain !== target.domain || !game.adapter || Boolean(registered?.installed && registered.enabled) || Boolean(detected)} onClick={() => void (registered ? registered.installed ? enable(registered.id) : onInstallLocal ? onInstallLocal(registered) : setLocalSelection(registered.id) : choose(target))}>{detected ? text.installed : registered ? registered.installed ? registered.enabled ? text.installed : text.enable : text.install : text.downloadNexus}</button>}</div></article>;
    })}</div></>}
    {busy && <p role="status" className="nexus-hint">{text.loading}</p>}
    {!selection && error && <p role="alert" className="nexus-error">{error}</p>}
    {message && <p role="status" className="nexus-hint">{message}</p>}
    {localSelection && game && <NexusInstallSequence game={game} modId={localSelection} busy={disabled || busy} errorMessage={error} onInstall={async (gameId, modId, selection) => {try {await invoke("install_nexus_mod", {gameId,modId,selection:selection ?? null}); return true;} catch(e) {setError(String(e)); return false;}}} onToggle={async (gameId, modId, enabled) => {try {await invoke("set_nexus_mod_enabled", {gameId,modId,enabled}); return true;} catch(e) {setError(String(e)); return false;}}} onClose={() => setLocalSelection(null)} onError={setError} />}
    {selection && game && <NexusDownloadQueue game={game} domain={selection.domain} modId={selection.id} details={selection.details} onClose={() => setSelection(null)} onStarted={() => { setSelection(null); setMessage(text.queueStarted); }} />}
  </section>;
}
