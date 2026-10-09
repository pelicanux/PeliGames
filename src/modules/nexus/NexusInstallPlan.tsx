import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusRequirements, type Requirements } from "./NexusRequirements";
import { useNexusText } from "./text";
import type { LocalMod, NexusGame } from "./useNexusWorkspace";
interface FomodOption { id: string; name: string; description: string; kind: string; selected: boolean }
interface FomodGroup { name: string; kind: string; options: FomodOption[] }
export interface Plan {
  notes: string[]; module: string; files: [string, string][]; resolved_files?: [string, string][];
  missing: { name: string; url: string; mod_id: number; kind: "nexus" | "external" }[];
  fomod?: { name: string; steps: { name: string; groups: FomodGroup[] }[] };
  selection: string[]; issues: string[]; activation_file?: string | null;
}
export function NexusInstallPlan({ game, modId, onReady, onSelection, onInstallLocal, requirements }: {
  game: NexusGame; modId: string; onReady: (ready: boolean) => void; onSelection: (selection: string[]) => void; onInstallLocal?: (mod: LocalMod) => void; requirements?: Requirements;
}) {
  const text = useNexusText();
  const [plan, setPlan] = useState<Plan | null>(null), [error, setError] = useState("");
  const [choices, setChoices] = useState<string[] | undefined>(), [loading, setLoading] = useState(true);
  const signature = JSON.stringify([game.mods.map(mod => [mod.id, mod.installed, mod.enabled]), game.detected_mods]);
  useEffect(() => {
    let active = true;
    onReady(false); setError(""); setLoading(true);
    void invoke<Plan>("plan_nexus_installation", { gameId: game.id, modId, selection: choices ?? null }).then(value => {
      if (active) { setPlan(value); onSelection(value.selection); onReady(!value.missing.length && !value.issues.length); }
    }).catch(error => { if (active) setError(String(error)); }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [game.id, modId, signature, choices, onReady, onSelection]);
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
    {loading && <p role="status" className="nexus-hint nexus-install-status">{text.loading}</p>}
    {plan && <>
      {plan.fomod && <section className="nexus-fomod" aria-label={text.fomodOptions}>
        <h3>{text.fomodOptions} · {plan.fomod.name}</h3>
        <p className="nexus-hint">{text.fomodHint}</p>
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
      {plan.issues.map(issue => <p role="alert" className="nexus-error" key={issue}>{issue}</p>)}
      <div className="nexus-install-summary"><span>{plan.module}</span><small>{plan.files.length} {text.plannedFiles}</small></div>
      {(plan.missing.length > 0 || requirements?.items.length || requirements && (!requirements.complete || requirements.error)) ? <NexusRequirements game={game} onInstallLocal={onInstallLocal} requirements={{complete: requirements?.complete ?? true, error: requirements?.error, items: Array.from(new Map< string, Requirements["items"][number] >([...plan.missing.map(item => ({name: item.name, url: item.url, kind: item.kind || "nexus", notes: text.moduleDependency})), ...(requirements?.items || [])].map(item => [`${item.kind}:${item.url || item.name}`, item])).values())}} onOpen={url => void openUrl(url).catch(error => setError(String(error)))} /> : null}
      {plan.notes?.length > 0 && <details className="nexus-install-notes"><summary>{text.installNotes}</summary>{plan.notes.map(note => <p className="nexus-hint" key={note}>{note}</p>)}</details>}
      {plan.activation_file && <p className="nexus-file-description">{text.activationFile}: {plan.activation_file}</p>}
      <details><summary>{text.installDestinations}</summary><div className="nexus-mod-list">{plan.files.map(([source, target]) => <div className="nexus-file-description" key={`${source}:${target}`}><small>{source}</small><div>→ {plan.resolved_files?.find(([file]) => file === source)?.[1] || target.replace(/^@local\//, `${text.prefixLocal}/`).replace(/^@documents\//, `${text.prefixDocuments}/`)}</div></div>)}</div></details>
    </>}
  </>;
}
