import {useI18n} from "../i18n/I18nContext";
// "Edit channel", Discord-style, on top of the room's Matrix state: what is
// saved here is what Element and the other clients see (and the other way
// round). Permissions are Matrix power levels shown as three roles.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SelectionMenu } from "./SelectionMenu";
import { displayName } from "../services/matrixChat";
import { channelLink, copyText } from "./MatrixChannelMenu";
import type { ChannelEditTab } from "./MatrixChannelList";

type Settings = {
  name: string; topic: string; join_rule: string; voice: boolean; my_power: number; users_default: number;
  permissions: Record<string, number>; users: [string, number][]; creators: string[];
  needs: { name: number; topic: number; join_rules: number; power_levels: number };
};
const MODERATOR = 50, ADMIN = 100;

export function MatrixChannelSettings({ room, names, initialTab, onClose, onError, onSaved }: {
  room: string; names: Record<string, string>; initialTab: ChannelEditTab;
  onClose: () => void; onError: (error: unknown, fallback: string) => void;
  /** The channel changed (name, topic): the list reloads. */
  onSaved: () => void;
}) {
  const {t: tx} = useI18n();

  const [tab, setTab] = useState<ChannelEditTab>(initialTab);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [failed, setFailed] = useState(false), [attempt, setAttempt] = useState(0);
  const reportError = useRef(onError); reportError.current = onError;
  const [saving, setSaving] = useState(false);
  const [notice, setNotice] = useState("");
  const [name, setName] = useState(""), [topic, setTopic] = useState(""), [joinRule, setJoinRule] = useState("");
  const [permissions, setPermissions] = useState<Record<string, number>>({});
  const [invitee, setInvitee] = useState(""), [newRole, setNewRole] = useState("");
  const frame = useRef<HTMLDivElement>(null);
  const load = (value: Settings) => {
    setSettings(value); setName(value.name); setTopic(value.topic); setJoinRule(value.join_rule); setPermissions(value.permissions);
  };
  useEffect(() => {
    let active = true;
    setFailed(false);
    void invoke<Settings>("matrix_channel_settings", { room }).then(value => { if (active) load(value); })
      .catch(error => { if (active) { setFailed(true); reportError.current(error, tx("ui-matrix-channel-settings", "could_not_load_the_channel")); } });
    return () => { active = false; };
  }, [room, attempt, tx]);
  useEffect(() => { frame.current?.querySelector<HTMLButtonElement>("nav button")?.focus(); }, []);
  const save = async (change: Record<string, unknown>, done: string) => {
    if (saving) return;
    setSaving(true); setNotice("");
    try { load(await invoke<Settings>("matrix_channel_update", { room, change })); setNotice(done); onSaved(); }
    catch (error) { onError(error, tx("ui-matrix-channel-settings", "matrix_did_not_accept_the_change")); }
    finally { setSaving(false); }
  };
  const mine = settings?.my_power ?? 0;
  const may = (need: number | undefined) => mine >= (need ?? ADMIN);
  // Three roles, as Discord shows them; another level set in Element stays listed as it is.
  const roleOptions = (current: number, base: number) => {
    const options = [{ value: String(base), label: tx("ui-matrix-channel-settings", "all_members") }, { value: String(MODERATOR), label: tx("ui-matrix-channel-settings", "moderators") }, { value: String(ADMIN), label: tx("ui-matrix-channel-settings", "administrators") }];
    if (!options.some(option => Number(option.value) === current)) options.push({ value: String(current), label: tx("ui-matrix-channel-settings", "level_from_element", {"current": current}) });
    return options.map(option => ({ ...option, disabled: Number(option.value) > mine }));
  };
  const memberOptions = (current: number, base: number) => {
    const options = [{ value: String(base), label: tx("ui-matrix-channel-settings", "member") }, { value: String(MODERATOR), label: tx("ui-matrix-channel-settings", "moderator") }, { value: String(ADMIN), label: tx("ui-matrix-channel-settings", "administrator") }];
    if (!options.some(option => Number(option.value) === current)) options.push({ value: String(current), label: tx("ui-matrix-channel-settings", "level", {"current": current}) });
    return options.map(option => ({ ...option, disabled: Number(option.value) >= mine && mine < 1_000_000 }));
  };
  const rows: [string, string, string][] = [
    ["send", tx("ui-matrix-channel-settings", "send_messages"), tx("ui-matrix-channel-settings", "write_in_the_channel_chat")],
    ["react", tx("ui-matrix-channel-settings", "add_reactions"), ""],
    ["voice", tx("ui-matrix-channel-settings", "connect_to_voice"), tx("ui-matrix-channel-settings", "join_the_call_and_share_screens_in_it")],
    ["invite", tx("ui-matrix-channel-settings", "invite_people"), ""],
    ["pin", tx("ui-matrix-channel-settings", "pin_messages"), ""],
    ["redact", tx("ui-matrix-channel-settings", "manage_messages"), tx("ui-matrix-channel-settings", "remove_other_people_s_messages")],
    ["kick", tx("ui-matrix-channel-settings", "kick_members"), ""],
    ["ban", tx("ui-matrix-channel-settings", "ban_members"), ""],
    ["manage", tx("ui-matrix-channel-settings", "manage_channel"), tx("ui-matrix-channel-settings", "name_topic_and_picture")],
    ["permissions", tx("ui-matrix-channel-settings", "manage_permissions"), tx("ui-matrix-channel-settings", "change_this_list_and_the_roles")],
  ];
  const changedPermissions = settings ? Object.fromEntries(Object.entries(permissions).filter(([id, level]) => settings.permissions[id] !== level)) : {};
  const tabs: [ChannelEditTab, string][] = [["overview", tx("ui-matrix-channel-settings", "overview")], ["permissions", tx("ui-matrix-channel-settings", "permissions")], ["roles", tx("ui-matrix-channel-settings", "roles")], ["invite", tx("ui-matrix-channel-settings", "invite")]];
  const canPermissions = may(settings?.needs.power_levels);
  return <div className="matrix-settings-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}
    onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); } }}>
    <div ref={frame} className="matrix-settings matrix-channel-settings" role="dialog" aria-modal="true" aria-label={tx("ui-matrix-channel-settings", "edit_channel")}>
      <header><strong>{tx("ui-matrix-channel-settings", "edit", {"settings_voice": settings?.voice ? "🔊 " : "#", "settings_name": settings?.name || ""})}</strong><button type="button" className="matrix-settings-close" onClick={onClose} aria-label={tx("ui-matrix-channel-settings", "close")}>[x]</button></header>
      <nav role="tablist">{tabs.map(([id, label]) => <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setNotice(""); }}>{label}</button>)}</nav>
      <div className="matrix-settings-body" role="tabpanel">
        {failed ? <button type="button" onClick={() => setAttempt(value => value + 1)}>{tx("ui-matrix-channel-settings", "try_again")}</button> : !settings ? <p className="matrix-settings-hint">{tx("ui-matrix-channel-settings", "reading_the_channel_from_matrix")}</p> : <>
          {tab === "overview" && <form className="matrix-channel-overview" onSubmit={event => {
            event.preventDefault();
            const change: Record<string, unknown> = {};
            if (name.trim() !== settings.name) change.name = name.trim();
            if (topic.trim() !== settings.topic) change.topic = topic.trim();
            if (joinRule !== settings.join_rule) change.join_rule = joinRule;
            if (Object.keys(change).length) void save(change, tx("ui-matrix-channel-settings", "channel_updated_on_matrix"));
          }}>
            <div className="matrix-settings-field"><label htmlFor="matrix-channel-name">{tx("ui-matrix-channel-settings", "channel_name")}</label>
              <input id="matrix-channel-name" value={name} maxLength={80} disabled={saving || !may(settings.needs.name)} onChange={event => setName(event.target.value)}/></div>
            <div className="matrix-settings-field"><label htmlFor="matrix-channel-topic">{tx("ui-matrix-channel-settings", "topic")}</label>
              <textarea id="matrix-channel-topic" value={topic} maxLength={1024} rows={3} disabled={saving || !may(settings.needs.topic)} onChange={event => setTopic(event.target.value)} placeholder={tx("ui-matrix-channel-settings", "what_this_channel_is_about")}/></div>
            <div className="matrix-settings-field"><span>{tx("ui-matrix-channel-settings", "who_can_join")}</span>
              <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-channel-settings", "who_can_join_2")} value={joinRule} disabled={saving || !may(settings.needs.join_rules)}
                onChange={setJoinRule} options={[
                  { value: "restricted", label: tx("ui-matrix-channel-settings", "community_members") },
                  { value: "invite", label: tx("ui-matrix-channel-settings", "invited_only_private_channel") },
                  { value: "public", label: tx("ui-matrix-channel-settings", "anyone_on_matrix") },
                  ...(["restricted", "invite", "public"].includes(settings.join_rule) ? [] : [{ value: settings.join_rule, label: settings.join_rule }]),
                ]}/></div>
            <small className="matrix-settings-hint">{tx("ui-matrix-channel-settings", "changes_the_channel_on_matrix_element_and_other_apps_show_the_same")}</small>
            <button type="submit" disabled={saving || (name.trim() === settings.name && topic.trim() === settings.topic && joinRule === settings.join_rule) || !name.trim()}>{saving ? (tx("ui-matrix-channel-settings", "saving")) : (tx("ui-matrix-channel-settings", "save"))}</button>
          </form>}
          {tab === "permissions" && <>
            <small className="matrix-settings-hint">{canPermissions ? (tx("ui-matrix-channel-settings", "who_may_do_each_thing_in_this_channel_you_can_set_up_to_your_own_role")) : (tx("ui-matrix-channel-settings", "your_role_can_view_but_not_change_this_channel_s_permissions"))}</small>
            <div className="matrix-permission-list">
              {rows.filter(([id]) => id in permissions).map(([id, label, hint]) => <div key={id} className="matrix-permission-row">
                <span>{label}{hint && <small>{hint}</small>}</span>
                <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={label} value={String(permissions[id])} disabled={saving || !canPermissions || settings.permissions[id] > mine}
                  options={roleOptions(permissions[id], settings.users_default)} onChange={value => setPermissions(current => ({ ...current, [id]: Number(value) }))}/>
              </div>)}
            </div>
            {canPermissions && <button type="button" disabled={saving || !Object.keys(changedPermissions).length} onClick={() => void save({ permissions: changedPermissions }, tx("ui-matrix-channel-settings", "permissions_saved_on_matrix"))}>{saving ? (tx("ui-matrix-channel-settings", "saving")) : (tx("ui-matrix-channel-settings", "save_permissions"))}</button>}
          </>}
          {tab === "roles" && <>
            <small className="matrix-settings-hint">{tx("ui-matrix-channel-settings", "roles_in_this_channel_channel_creators_have_full_power_and_are_not_listed_by_level")}</small>
            <ul className="matrix-role-list">
              {settings.creators.map(user => <li key={user}><span>{displayName(user, names)}<small>{user}</small></span><b>{tx("ui-matrix-channel-settings", "creator")}</b></li>)}
              {settings.users.map(([user, level]) => <li key={user}><span>{displayName(user, names)}<small>{user}</small></span>
                <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={tx("ui-matrix-channel-settings", "s_role", {"displayname_user_names": displayName(user, names)})} value={String(level)} disabled={saving || !canPermissions || (level >= mine && mine < 1_000_000)}
                  options={memberOptions(level, settings.users_default)} onChange={value => void save({ users: { [user]: Number(value) === settings.users_default ? null : Number(value) } }, tx("ui-matrix-channel-settings", "role_updated"))}/>
              </li>)}
            </ul>
            {canPermissions && <form className="matrix-settings-field" onSubmit={event => { event.preventDefault(); const user = newRole.trim(); if (user) void save({ users: { [user]: MODERATOR } }, tx("ui-matrix-channel-settings", "moderator_added")).then(() => setNewRole("")); }}>
              <label htmlFor="matrix-role-user">{tx("ui-matrix-channel-settings", "make_moderator")}</label>
              <div><input id="matrix-role-user" value={newRole} maxLength={255} placeholder="@nome:matrix.org" disabled={saving} onChange={event => setNewRole(event.target.value)}/>
                <button type="submit" disabled={saving || !newRole.trim().startsWith("@")}>{tx("ui-matrix-channel-settings", "add")}</button></div>
            </form>}
          </>}
          {tab === "invite" && <>
            <form className="matrix-settings-field" onSubmit={event => {
              event.preventDefault();
              const user = invitee.trim();
              if (!user || saving) return;
              setSaving(true); setNotice("");
              void invoke("matrix_channel_invite", { room, user }).then(() => { setNotice(tx("ui-matrix-channel-settings", "invite_sent_to", {"user": user})); setInvitee(""); })
                .catch(error => onError(error, tx("ui-matrix-channel-settings", "could_not_invite"))).finally(() => setSaving(false));
            }}>
              <label htmlFor="matrix-invite-user">{tx("ui-matrix-channel-settings", "invite_a_matrix_account")}</label>
              <div><input id="matrix-invite-user" value={invitee} maxLength={255} placeholder="@nome:matrix.org" disabled={saving} onChange={event => setInvitee(event.target.value)}/>
                <button type="submit" disabled={saving || !invitee.trim().startsWith("@")}>{saving ? (tx("ui-matrix-channel-settings", "sending")) : (tx("ui-matrix-channel-settings", "invite_2"))}</button></div>
              <small>{tx("ui-matrix-channel-settings", "the_person_gets_an_invite_to_the_channel_and_the_community")}</small>
            </form>
            <div className="matrix-settings-field"><span>{tx("ui-matrix-channel-settings", "channel_link")}</span>
              <div><code>{channelLink(room)}</code><button type="button" onClick={() => void copyText(channelLink(room)).then(() => setNotice(tx("ui-matrix-channel-settings", "link_copied"))).catch(error => onError(error, tx("ui-matrix-channel-settings", "could_not_copy")))}>{tx("ui-matrix-channel-settings", "copy")}</button></div>
              <small>{tx("ui-matrix-channel-settings", "opens_the_channel_in_element_or_another_matrix_app_people_outside_the_community_need_an_")}</small>
            </div>
          </>}
        </>}
        {notice && <p className="matrix-settings-notice" role="status">{notice}</p>}
      </div>
    </div>
  </div>;
}
