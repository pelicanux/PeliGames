import { useEffect, useId, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { useBoundedDropdown } from "../../hooks/useBoundedDropdown";
import type { DeployMethod, NexusGame } from "./useNexusWorkspace";
import { useNexusText } from "./text";
interface Option { method: DeployMethod; available: boolean; reason: string }
export function NexusDeployOptions({ game, disabled, onChange, onSaved }: {game: NexusGame; disabled: boolean; onChange: (method: DeployMethod) => Promise<boolean>; onSaved: () => void}) {
  const text = useNexusText();
  const [options,setOptions] = useState<Option[]>([]);
  const [saving,setSaving] = useState(false), [open,setOpen] = useState(false);
  const [error,setError] = useState("");
  const anchor = useRef<HTMLDivElement>(null), menu = useRef<HTMLDivElement>(null);
  const id = useId();
  const currentGame = useRef(game.id); currentGame.current = game.id;
  const position = useBoundedDropdown(open, anchor, menu, setOpen, 220);
  useEffect(() => {
    let active = true; setOptions([]); setError(""); setOpen(false);
    void invoke<Option[]>("nexus_deploy_options",{gameId:game.id}).then(values => {if(active)setOptions(values);}).catch(e => {if(active)setError(String(e));});
    return () => {active = false;};
  },[game.id, game.game.directory]);
  useEffect(() => {if(disabled) setOpen(false);},[disabled]);
  const selected = game.deploy_method || "copy";
  const labels: Record<DeployMethod,string> = {copy:"Copy",symlink:"Symlink",hardlink:"Hardlink",vfs:"VFS"};
  const hints: Record<DeployMethod,string> = {copy:text.deployCopyHint,symlink:text.deploySymlinkHint,hardlink:text.deployHardlinkHint,vfs:text.deployVfsHint};
  const info = (method: DeployMethod, reason?: string) => <HoverTooltip text={reason || hints[method]}><button type="button" className="installation-type-info" aria-label={`${text.deployMethod}: ${labels[method]}`}><MenuIcon name="info" /></button></HoverTooltip>;
  return <div className="installation-field nexus-deploy-selector">
    <div className="installation-type-label"><span className="installation-field-label">{text.deployMethod}</span>{info(selected)}</div>
    <div ref={anchor}><button type="button" className={`custom-select-trigger proton-trigger nexus-compact-select ${open ? "open" : ""}`} aria-label={text.deployMethod} aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? id : undefined} disabled={disabled || saving || !options.length} onClick={() => setOpen(value => !value)}><span>{labels[selected]}</span><span className="select-arrow"><svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" aria-hidden="true"><path d="m6 9 6 6 6-6" /></svg></span></button></div>
    {open && createPortal(<div ref={menu} id={id} role="listbox" aria-label={text.deployMethod} className="custom-select-menu route-select-menu nexus-deploy-menu nexus-compact-select-menu" style={position} onKeyDown={event => {
      if(event.key === "Escape") {setOpen(false); anchor.current?.querySelector<HTMLButtonElement>("button")?.focus();}
      if(event.key === "Tab") setOpen(false);
      if(event.key === "ArrowDown" || event.key === "ArrowUp") {event.preventDefault(); const buttons = Array.from(menu.current?.querySelectorAll<HTMLButtonElement>('button[role="option"]:not(:disabled)') || []); const index = buttons.indexOf(document.activeElement as HTMLButtonElement); buttons[(index + (event.key === "ArrowDown" ? 1 : -1) + buttons.length) % buttons.length]?.focus();}
    }}>{options.map(option => <div className="nexus-deploy-menu-row" key={option.method}><button type="button" className={`custom-select-option ${selected === option.method ? "selected" : ""}`} role="option" aria-selected={selected === option.method} autoFocus={selected === option.method} disabled={!option.available} onClick={() => {
      setOpen(false); anchor.current?.querySelector<HTMLButtonElement>("button")?.focus();
      if(selected === option.method) return;
      const gameId = game.id; setSaving(true); void onChange(option.method).then(ok => {if(ok && currentGame.current === gameId) onSaved();}).finally(() => setSaving(false));
    }}>{labels[option.method]}</button>{info(option.method, option.available ? undefined : option.reason)}</div>)}</div>,document.body)}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </div>;
}
