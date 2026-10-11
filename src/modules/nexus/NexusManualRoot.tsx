import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";

export interface ManualRoot { source: string; destination: string }
interface PackageLayout { folders: string[]; destinations: string[]; current?: ManualRoot | null }

/** MO2-style "set data directory" for packages whose structure is not recognized. */
export function NexusManualRoot({ game, modId, onChanged }: { game: NexusGame; modId: string; onChanged: () => void }) {
  const text = useNexusText();
  const [layout, setLayout] = useState<PackageLayout | null>(null);
  const [open, setOpen] = useState(false), [saving, setSaving] = useState(false), [error, setError] = useState("");
  const [source, setSource] = useState(""), [destination, setDestination] = useState("");
  useEffect(() => {
    let active = true;
    void invoke<PackageLayout>("nexus_package_layout", { gameId: game.id, modId }).then(value => {
      if (!active) return;
      setLayout(value);
      setSource(value.current?.source ?? ""); setDestination(value.current?.destination ?? "");
    }).catch(error => { if (active) setError(String(error)); });
    return () => { active = false; };
  }, [game.id, modId]);
  const apply = (root: ManualRoot | null) => {
    setSaving(true); setError("");
    void invoke("set_nexus_manual_root", { gameId: game.id, modId, root }).then(() => {
      setLayout(value => value && { ...value, current: root }); setOpen(false); onChanged();
    }).catch(error => setError(String(error))).finally(() => setSaving(false));
  };
  const current = layout?.current;
  const folderLabel = (folder: string) => folder ? folder.replace(/\/$/, "") : text.manualRootArchive;
  const destinationLabel = (value: string) => value || text.manualRootAutomatic;
  return <section className={open ? "nexus-fomod nexus-manual-root" : "nexus-manual-root"} aria-label={text.manualRoot}>
    {current && <p className="nexus-hint">{text.manualRootActive}: <strong>{folderLabel(current.source)}</strong> → <strong>{destinationLabel(current.destination)}</strong></p>}
    {!open && <div className="nexus-manual-root-actions">
      <button type="button" className="btn btn-secondary" disabled={!layout || saving} onClick={() => setOpen(true)}>{current ? text.manualRootChange : text.manualRoot}</button>
      {current && <button type="button" className="btn btn-secondary" disabled={saving} onClick={() => apply(null)}>{text.manualRootClear}</button>}
    </div>}
    {open && layout && <>
      <h3>{text.manualRoot}</h3>
      <p className="nexus-hint">{text.manualRootHint}</p>
      <fieldset><legend>{text.manualRootFolder}</legend>
        <div className="nexus-manual-root-folders">{layout.folders.map(folder => <label key={folder || "."} className={`nexus-fomod-option ${source === folder ? "selected" : ""}`}>
          <input type="radio" name="nexus-manual-root-source" checked={source === folder} onChange={() => setSource(folder)} />
          <span>{folderLabel(folder)}</span>
        </label>)}</div>
      </fieldset>
      <fieldset><legend>{text.manualRootDestination}</legend>
        {["", ...layout.destinations].map(value => <label key={value || "."} className={`nexus-fomod-option ${destination === value ? "selected" : ""}`}>
          <input type="radio" name="nexus-manual-root-destination" checked={destination === value} onChange={() => setDestination(value)} />
          <span><strong>{destinationLabel(value)}</strong>{!value && <small>{text.manualRootAutomaticHint}</small>}</span>
        </label>)}
      </fieldset>
      <div className="nexus-manual-root-actions">
        <button type="button" className="btn btn-primary" disabled={saving} onClick={() => apply({ source, destination })}>{text.manualRootApply}</button>
        <button type="button" className="btn btn-secondary" disabled={saving} onClick={() => setOpen(false)}>{text.cancel}</button>
      </div>
    </>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </section>;
}
