import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusManualRoot } from "./NexusManualRoot";
import { NexusRequirements, type Requirements } from "./NexusRequirements";
import { useNexusText } from "./text";
import type { LocalMod, NexusGame } from "./useNexusWorkspace";
interface FomodOption { id: string; name: string; description: string; kind: string; selected: boolean }
interface FomodGroup { name: string; kind: string; options: FomodOption[] }
export interface Plan {
  notes: string[]; module: string; files: [string, string][]; resolved_files?: [string, string][];
  missing: { name: string; url: string; mod_id: number; kind: "nexus" | "external" }[];
  fomod?: { name: string; steps: { name: string; groups: FomodGroup[] }[] };
  selection: string[]; issues: string[]; originals?: string[]; replace_originals?: boolean; activation_file?: string | null; overrides?: [string, string][]; manual_root?: { source: string; destination: string } | null;
}
export function NexusInstallPlan({ game, modId, onReady, onSelection, onInstallLocal, requirements, onLoadingChange }: {
  game: NexusGame; modId: string; onReady: (ready: boolean) => void; onSelection: (selection: string[]) => void; onInstallLocal?: (mod: LocalMod) => void; requirements?: Requirements; onLoadingChange?: (loading: boolean) => void;
}) {
  const text = useNexusText();
  const [plan, setPlan] = useState<Plan | null>(null), [error, setError] = useState("");
  const [choices, setChoices] = useState<string[] | undefined>(), [loading, setLoading] = useState(true);
  // Re-plans after the manual root changes; the root itself is stored by the backend.
  const [revision, setRevision] = useState(0);
  const [replacing, setReplacing] = useState(false);
  const signature = JSON.stringify([game.mods.map(mod => [mod.id, mod.installed, mod.enabled]), game.detected_mods]);
  useEffect(() => {
    let active = true;
    onReady(false); setError(""); setLoading(true); onLoadingChange?.(true);
    void invoke<Plan>("plan_nexus_installation", { gameId: game.id, modId, selection: choices ?? null }).then(value => {
      // A replacer needs the player's approval, like the Vortex "Install as Replacer" dialog.
      if (active) { setPlan(value); onSelection(value.selection); onReady(!value.missing.length && !value.issues.length && (!value.originals?.length || Boolean(value.replace_originals))); }
    }).catch(error => { if (active) setError(String(error)); }).finally(() => { if (active) { setLoading(false); onLoadingChange?.(false); } });
    return () => { active = false; onLoadingChange?.(false); };
  }, [game.id, modId, signature, choices, revision, onReady, onSelection, onLoadingChange]);
  const mod = game.mods.find(item => item.id === modId);
  // BAIN packages reuse the FOMOD selection flow (backend installer named "BAIN").
  const bain = Boolean(plan?.fomod?.name.startsWith("BAIN"));
  const unrecognized = Boolean(error) || Boolean(plan && !plan.fomod && (plan.issues.length > 0 || plan.files.length === 0));
  const choose = (group: FomodGroup, option: FomodOption, checked: boolean) => {
    if (!plan) return;
    onReady(false);
    const selected = new Set(choices ?? plan.selection);
    if (group.kind === "SelectExactlyOne" || group.kind === "SelectAtMostOne") group.options.forEach(item => selected.delete(item.id));
    if (checked) selected.add(option.id); else selected.delete(option.id);
    setChoices(Array.from(selected));
  };
  return <>
    {error && <p role="alert" className="nexus-error">{error}</p>}
    {loading && !onLoadingChange && <p role="status" className="nexus-hint nexus-install-status">{text.loading}</p>}
    {!loading && !plan && error && !mod?.installed && <NexusManualRoot game={game} modId={modId} onChanged={() => setRevision(value => value + 1)} />}
    {plan && <>
      {plan.fomod && <section className="nexus-fomod" aria-label={bain ? text.bainOptions : text.fomodOptions}>
        <h3>{bain ? text.bainOptions : `${text.fomodOptions} · ${plan.fomod.name}`}</h3>
        <p className="nexus-hint">{bain ? text.bainHint : text.fomodHint}</p>
        {plan.fomod.steps.map((step, stepIndex) => <section key={stepIndex}><h4>{step.name}</h4>
          {step.groups.map((group, groupIndex) => <fieldset key={groupIndex}>
            <legend>{group.name} · {group.kind === "SelectExactlyOne" ? text.fomodExactlyOne : group.kind === "SelectAtLeastOne" ? text.fomodAtLeastOne : group.kind === "SelectAtMostOne" ? text.fomodAtMostOne : group.kind === "SelectAll" ? text.fomodAll : text.fomodAny}</legend>
            {group.options.map(option => {
              const selected = choices ? option.kind !== "NotUsable" && (choices.includes(option.id) || group.kind === "SelectAll" || option.kind === "Required") : option.selected;
              return <label key={option.id} className={`nexus-fomod-option ${selected ? "selected" : ""}`}>
              <input type={group.kind === "SelectExactlyOne" ? "radio" : "checkbox"} name={`fomod-${stepIndex}-${groupIndex}`}
                checked={selected} disabled={group.kind === "SelectAll" || option.kind === "Required" || option.kind === "NotUsable"}
                onChange={event => choose(group, option, event.target.checked)} />
              <span><strong>{option.name}</strong>{option.kind === "Required" && <small>{text.fomodRequired}</small>}{option.kind === "NotUsable" && <small>{text.fomodUnavailable}</small>}{option.description && <p className="nexus-file-description">{option.description}</p>}</span>
            </label>; })}
          </fieldset>)}
        </section>)}
      </section>}
      {Boolean(plan.originals?.length) && !mod?.installed && <section className="nexus-requirements-warning nexus-replacer" aria-label={text.replacerTitle}>
        <h4>{text.replacerTitle}</h4>
        <p className="nexus-hint">{text.replacerHint}</p>
        <details><summary>{text.replacerFiles}: {plan.originals!.length}</summary><div className="nexus-mod-list">{plan.originals!.map(target => <div className="nexus-file-description" key={target}>{target}</div>)}</div></details>
        <label className={`nexus-fomod-option ${plan.replace_originals ? "selected" : ""}`}><input type="checkbox" checked={Boolean(plan.replace_originals)} disabled={replacing} onChange={event => { const enabled = event.target.checked; setReplacing(true); onReady(false); void invoke("set_nexus_mod_replace_originals", { gameId: game.id, modId, enabled }).then(() => setRevision(value => value + 1)).catch(e => setError(String(e))).finally(() => setReplacing(false)); }} /><span><strong>{text.replacerInstall}</strong><p className="nexus-file-description">{text.replacerBackup}</p></span></label>
      </section>}
      {plan.issues.map(issue => <p role="alert" className="nexus-error" key={issue}>{issue}</p>)}
      {!loading && !mod?.installed && (unrecognized || plan.manual_root) && <NexusManualRoot game={game} modId={modId} onChanged={() => setRevision(value => value + 1)} />}
      {Boolean(plan.overrides?.length) && <details className="nexus-requirements-warning"><summary>{text.planOverrides}: {plan.overrides!.length} · {Array.from(new Set(plan.overrides!.map(([, owner]) => owner))).join(", ")}</summary><div className="nexus-mod-list">{plan.overrides!.map(([target, owner]) => <div className="nexus-file-description" key={`${target}:${owner}`}><small>{owner}</small><div>{target}</div></div>)}</div><p className="nexus-hint">{text.planOverridesHint}</p></details>}
      <div className="nexus-install-summary"><span>{plan.module}</span><small>{plan.files.length} {text.plannedFiles}</small></div>
      {(plan.missing.length > 0 || requirements?.items.length || requirements && (!requirements.complete || requirements.error)) ? <NexusRequirements game={game} onInstallLocal={onInstallLocal} requirements={{complete: requirements?.complete ?? true, error: requirements?.error, items: Array.from(new Map< string, Requirements["items"][number] >([...plan.missing.map(item => ({name: item.name, url: item.url, kind: item.kind || "nexus", notes: text.moduleDependency})), ...(requirements?.items || [])].map(item => [`${item.kind}:${item.url || item.name}`, item])).values())}} onOpen={url => void openUrl(url).catch(error => setError(String(error)))} /> : null}
      {plan.notes?.length > 0 && <details className="nexus-install-notes"><summary>{text.installNotes}</summary>{plan.notes.map(note => <p className="nexus-hint" key={note}>{note}</p>)}</details>}
      {plan.activation_file && <p className="nexus-file-description">{text.activationFile}: {plan.activation_file}</p>}
      <details><summary>{text.installDestinations}</summary><div className="nexus-mod-list">{plan.files.map(([source, target]) => <div className="nexus-file-description" key={`${source}:${target}`}><small>{source}</small><div>→ {plan.resolved_files?.find(([file]) => file === source)?.[1] || target.replace(/^@local\//, `${text.prefixLocal}/`).replace(/^@documents\//, `${text.prefixDocuments}/`)}</div></div>)}</div></details>
    </>}
  </>;
}
