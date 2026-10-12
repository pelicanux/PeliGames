import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import type { NexusGame } from "./useNexusWorkspace";
export interface ProfileSummary { id: string; name: string; active: boolean; enabled: number; own_saves: boolean; own_settings: boolean; saves_supported: boolean }
/** Mod profiles of a game (Vortex profiles): create, rename, delete and switch. */
export function NexusProfiles({ game, profiles, disabled, onChange, onSwitch, onClose }: {
  game: NexusGame; profiles: ProfileSummary[]; disabled: boolean;
  onChange: (profiles: ProfileSummary[]) => void; onSwitch: (id: string) => Promise<boolean>; onClose: () => void;
}) {
  const text = useNexusText();
  const [name, setName] = useState("");
  const [copy, setCopy] = useState(true);
  const [editing, setEditing] = useState<{ id: string; name: string } | null>(null);
  const [removing, setRemoving] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const run = async (action: () => Promise<ProfileSummary[] | boolean>) => {
    if (working) return;
    setWorking(true); setError("");
    try { const value = await action(); if (Array.isArray(value)) onChange(value); }
    catch (e) { setError(String(e)); }
    finally { setWorking(false); }
  };
  const locked = disabled || working;
  const createForm = <form className="nexus-profile-create" onSubmit={event => { event.preventDefault(); if (!locked && name.trim()) void run(() => invoke<ProfileSummary[]>("create_nexus_user_profile", { gameId: game.id, name, copy })).then(() => setName("")); }}>
      <label className="nexus-profile-name-field"><span>{text.profileName}</span>
        <input type="text" className="nexus-profile-name-input" placeholder={text.profileName} value={name} disabled={locked} onChange={event => setName(event.target.value)} />
      </label>
      <div className="nexus-profile-create-actions">
        <label className="preference-switch nexus-profile-toggle"><input type="checkbox" checked={copy} disabled={locked} onChange={event => setCopy(event.target.checked)} /><span className="preference-switch-track"/><span>{text.profileCopy}</span></label>
        <button type="submit" className="btn btn-primary" disabled={locked || !name.trim()}><MenuIcon name="plusCircle" />{text.profileCreate}</button>
      </div>
    </form>;
  return <NexusPopup className="nexus-profiles-popup" title={text.profiles} onClose={onClose} footer={createForm}>
    <p className="nexus-hint">{text.profilesHint}</p>
    <ol className="nexus-profile-list">{profiles.map(profile => <li key={profile.id} className={`nexus-profile-row${profile.active ? " is-active" : ""}`}>
      <span className="nexus-profile-status" aria-hidden="true">{profile.active ? <MenuIcon name="check" /> : ""}</span>
      <span className="nexus-installed-mod-info">
        {editing?.id === profile.id
          ? <input type="text" className="nexus-profile-name-input" aria-label={text.profileName} value={editing.name} autoFocus disabled={locked} onChange={event => setEditing({ id: profile.id, name: event.target.value })}
              onKeyDown={event => { if (event.key === "Enter") void run(() => invoke<ProfileSummary[]>("rename_nexus_user_profile", { gameId: game.id, id: profile.id, name: editing.name })).then(() => setEditing(null)); if (event.key === "Escape") { event.stopPropagation(); setEditing(null); } }} />
          : <span className="nexus-mod-title"><strong>{profile.name}</strong></span>}
        <small>{profile.active ? text.profileActive : text.profileInactive} · {text.profileEnabled}: {profile.enabled}</small>
        {removing === profile.id && <small className="nexus-error">{text.profileRemovePrompt}</small>}
        {profile.saves_supported && <span className="nexus-profile-options">
          <label className="preference-switch nexus-profile-toggle"><input type="checkbox" checked={profile.own_saves} disabled={locked} onChange={event => void run(() => invoke<ProfileSummary[]>("set_nexus_profile_options", { gameId: game.id, id: profile.id, ownSaves: event.target.checked, ownSettings: profile.own_settings }))} /><span className="preference-switch-track"/><span>{text.profileOwnSaves}</span></label>
          <label className="preference-switch nexus-profile-toggle"><input type="checkbox" checked={profile.own_settings} disabled={locked} onChange={event => void run(() => invoke<ProfileSummary[]>("set_nexus_profile_options", { gameId: game.id, id: profile.id, ownSaves: profile.own_saves, ownSettings: event.target.checked }))} /><span className="preference-switch-track"/><span>{text.profileOwnSettings}</span></label>
        </span>}
      </span>
      <span className="nexus-profile-actions">
        {!profile.active && <button type="button" className="btn btn-primary" disabled={locked} onClick={() => void run(() => onSwitch(profile.id))}><MenuIcon name="play" />{working ? text.loading : text.profileSwitch}</button>}
        <button type="button" className="btn btn-secondary nexus-profile-icon" aria-label={`${text.profileRename}: ${profile.name}`} title={text.profileRename} disabled={locked} onClick={() => setEditing({ id: profile.id, name: profile.name })}><MenuIcon name="edit" /></button>
        {!profile.active && <button type="button" className="btn btn-secondary nexus-profile-icon" aria-label={`${text.remove}: ${profile.name}`} title={text.remove} disabled={locked}
          onClick={() => { if (removing === profile.id) void run(() => invoke<ProfileSummary[]>("delete_nexus_user_profile", { gameId: game.id, id: profile.id })).then(() => setRemoving(null)); else setRemoving(profile.id); }}><MenuIcon name="trash" /></button>}
      </span>
    </li>)}</ol>
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </NexusPopup>;
}
