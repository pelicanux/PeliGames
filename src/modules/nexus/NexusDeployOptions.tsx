import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HoverTooltip } from "../../components/HoverTooltip";
import type { DeployMethod, NexusGame } from "./useNexusWorkspace";
import { useNexusText } from "./text";
interface Option { method: DeployMethod; available: boolean; reason: string }
export function NexusDeployOptions({ game, disabled, onChange, onSaved }: {game: NexusGame; disabled: boolean; onChange: (method: DeployMethod) => Promise<boolean>; onSaved: () => void}) {
  const text = useNexusText();
  const [options,setOptions] = useState<Option[]>([]);
  const [saving,setSaving] = useState(false);
  const [error,setError] = useState("");
  const currentGame = useRef(game.id); currentGame.current = game.id;
  useEffect(() => {
    let active = true; setOptions([]); setError("");
    void invoke<Option[]>("nexus_deploy_options",{gameId:game.id}).then(values => {if(active)setOptions(values);}).catch(e => {if(active)setError(String(e));});
    return () => {active = false;};
  },[game.id, game.game.directory]);
  const selected = game.deploy_method || "copy";
  const labels: Record<DeployMethod,string> = {copy:"Copy",symlink:"Symlink",hardlink:"Hardlink",vfs:"VFS"};
  const hints: Record<DeployMethod,string> = {copy:text.deployCopyHint,symlink:text.deploySymlinkHint,hardlink:text.deployHardlinkHint,vfs:text.deployVfsHint};
  return <div className="installation-field nexus-deploy-selector">
    <div className="installation-type-label"><span className="installation-field-label">{text.deployMethod}</span></div>
    <div className="modern-dlss-segments modern-deploy-segments" role="group" aria-label={text.deployMethod}>
      {options.map(option => <HoverTooltip key={option.method} text={option.available ? hints[option.method] : option.reason || hints[option.method]}>
        <button type="button" aria-pressed={selected === option.method} disabled={disabled || saving || !option.available} onClick={() => {
          if (selected === option.method) return;
          const gameId = game.id; setSaving(true);
          void onChange(option.method).then(ok => { if (ok && currentGame.current === gameId) onSaved(); }).finally(() => setSaving(false));
        }}>{labels[option.method]}</button>
      </HoverTooltip>)}
      {!options.length && !error && <span className="nexus-hint">{text.loading}</span>}
    </div>
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </div>;
}
