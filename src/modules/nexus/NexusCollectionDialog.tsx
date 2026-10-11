import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";

interface CollectionMod { mod_id: number; file_id: number; name: string; file_name: string; size?: number | null; version: string; optional: boolean; present: boolean }
interface CollectionPlan { domain: string; slug: string; revision: number; name: string; mods: CollectionMod[] }
interface CollectionQueued { queued: number; present: number; manifest_error?: string | null }

const size = (bytes?: number | null) => bytes ? bytes >= 1048576 ? `${(bytes / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1024))} KB` : "";

/** Nexus Collection: link → revision preview → queue required and chosen optional files. */
export function NexusCollectionDialog({ game, initialReference, onClose, onQueued }: { game: NexusGame; initialReference?: string; onClose: () => void; onQueued: () => void }) {
  const text = useNexusText();
  const [reference, setReference] = useState(initialReference ?? "");
  const [plan, setPlan] = useState<CollectionPlan | null>(null);
  const [optional, setOptional] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false), [error, setError] = useState(""), [result, setResult] = useState<CollectionQueued | null>(null);
  const load = (value: string) => {
    if (!value.trim()) return;
    setBusy(true); setError(""); setPlan(null); setResult(null);
    void invoke<CollectionPlan>("load_nexus_collection", { gameId: game.id, reference: value.trim() })
      .then(value => { setPlan(value); setOptional(new Set()); })
      .catch(error => setError(String(error))).finally(() => setBusy(false));
  };
  useEffect(() => { if (initialReference) load(initialReference); }, [initialReference]); // eslint-disable-line react-hooks/exhaustive-deps
  const queue = () => {
    if (!plan) return;
    setBusy(true); setError("");
    // The loaded revision is installed, even if a newer one is published meanwhile.
    void invoke<CollectionQueued>("install_nexus_collection", { gameId: game.id, reference: `https://www.nexusmods.com/games/${plan.domain}/collections/${plan.slug}/revisions/${plan.revision}`, optional: Array.from(optional) })
      .then(value => { setResult(value); onQueued(); })
      .catch(error => setError(String(error))).finally(() => setBusy(false));
  };
  const required = plan?.mods.filter(mod => !mod.optional) ?? [], optionalMods = plan?.mods.filter(mod => mod.optional) ?? [];
  const pending = plan ? plan.mods.filter(mod => !mod.present && (!mod.optional || optional.has(mod.file_id))).length : 0;
  const row = (mod: CollectionMod) => <span><strong>{mod.name}</strong>{mod.version && <small> v{mod.version}</small>}<small>{[mod.file_name, size(mod.size), mod.present ? text.collectionPresent : ""].filter(Boolean).join(" · ")}</small></span>;
  return <NexusPopup className="nexus-collection-popup" title={text.collection} onClose={onClose} footer={<>
    <button type="button" className="btn btn-secondary" disabled={busy} onClick={onClose}>{result ? text.close : text.cancel}</button>
    {plan && !result && <button type="button" className="btn btn-primary" disabled={busy || pending === 0} onClick={queue}><MenuIcon name="download" />{busy ? text.loading : `${text.collectionInstall} (${pending})`}</button>}
  </>}>
    <form className="nexus-fields" onSubmit={event => { event.preventDefault(); load(reference); }}>
      <label htmlFor="nexus-collection-reference">{text.collectionLink}<input id="nexus-collection-reference" value={reference} onChange={event => setReference(event.target.value)} placeholder="https://www.nexusmods.com/games/…/collections/…" spellCheck={false} autoCapitalize="none" maxLength={2048} disabled={busy} /></label>
      <button type="submit" className="btn btn-secondary" disabled={busy || !reference.trim()}><MenuIcon name="search" />{text.collectionLoad}</button>
    </form>
    <p className="nexus-hint">{text.collectionHint}</p>
    {busy && !plan && <p role="status" className="nexus-hint">{text.loading}</p>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
    {plan && <section className="nexus-fomod" aria-label={plan.name}>
      <h3>{plan.name} · {text.collectionRevision} {plan.revision}</h3>
      <p className="nexus-hint">{required.length} {text.collectionRequired} · {optionalMods.length} {text.collectionOptional}</p>
      <fieldset><legend>{text.collectionRequired}</legend>
        <div className="nexus-manual-root-folders">{required.map(mod => <div key={mod.file_id} className={`nexus-fomod-option ${mod.present ? "" : "selected"}`}><input type="checkbox" checked readOnly disabled aria-label={mod.name} />{row(mod)}</div>)}</div>
      </fieldset>
      {optionalMods.length > 0 && <fieldset><legend>{text.collectionOptional}</legend>
        <div className="nexus-manual-root-folders">{optionalMods.map(mod => <label key={mod.file_id} className={`nexus-fomod-option ${optional.has(mod.file_id) ? "selected" : ""}`}>
          <input type="checkbox" checked={mod.present || optional.has(mod.file_id)} disabled={mod.present || busy || Boolean(result)} onChange={event => setOptional(current => { const next = new Set(current); if (event.target.checked) next.add(mod.file_id); else next.delete(mod.file_id); return next; })} />{row(mod)}
        </label>)}</div>
      </fieldset>}
      <p className="nexus-hint">{text.collectionLimits}</p>
    </section>}
    {result && <div role="status">
      <p className="nexus-hint"><strong>{result.queued}</strong> {text.collectionQueued}{result.present ? ` · ${result.present} ${text.collectionPresent}` : ""}</p>
      {result.manifest_error && <p className="nexus-error">{text.collectionManifestError}: {result.manifest_error}</p>}
    </div>}
  </NexusPopup>;
}
