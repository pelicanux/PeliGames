import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { NexusRequirements } from "./NexusRequirements";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
interface Report { notes: string[]; name?: string; domain?: string; module_version?: string; mod_destinations: string[]; isolated_profile: boolean; frameworks: {name:string;url:string;installed:boolean;kind:"nexus"|"external"}[] }
export function NexusModuleStatus({ game }: { game?: NexusGame }) {
  const text = useNexusText();
  const [report, setReport] = useState<Report | null>(null);
  const [errors, setErrors] = useState<string[]>([]);
  const signature = JSON.stringify([game?.mods.map(mod => [mod.id,mod.enabled,mod.installed]), game?.detected_mods]);
  useEffect(() => {
    let active = true;
    setReport(null); setErrors([]);
    if (game?.game.directory) {
      void invoke<Report>("inspect_nexus_game", { source: game.game.directory }).then(value => { if (active) setReport(value); }).catch(error => { if (active) setErrors(previous => [...previous, String(error)]); });
    }
    void invoke<{ errors: string[] }>("list_nexus_modules").then(value => { if (active) setErrors(previous => [...previous, ...value.errors]); }).catch(error => { if (active) setErrors(previous => [...previous, String(error)]); });
    return () => { active = false; };
  }, [game?.id, game?.game.directory, signature]);
  return <><dl className="nexus-info"><dt>{text.moduleLabel}</dt><dd>{report?.domain ? `${report.name} · ${report.module_version}` : text.moduleMissing}</dd>{report?.isolated_profile && <><dt>{text.moduleDestinations}</dt><dd>{text.isolatedModule}</dd></>}{Boolean(report?.mod_destinations.length) && <><dt>{text.moduleDestinations}</dt><dd>{report!.mod_destinations.map(path => <div key={path}>{path}</div>)}</dd></>}</dl>{Boolean(report?.frameworks.length) && <NexusRequirements game={game} requirements={{complete:true,items:report!.frameworks.map(item => ({name:item.name,url:item.url,kind:item.kind || "nexus",notes:item.installed ? text.installed : text.moduleDependency}))}} onOpen={url => void openUrl(url).catch(error => setErrors(previous => [...previous,String(error)]))} />}{report?.notes?.map(note => <p className="nexus-hint" key={note}>{note}</p>)}{errors.length > 0 && <div role="alert" className="nexus-error"><strong>{text.moduleErrors}</strong>{errors.map((error, index) => <p key={index}>{error}</p>)}</div>}</>;
}
