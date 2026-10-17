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

export function MatrixChannelSettings({ pt, room, names, initialTab, onClose, onError, onSaved }: {
  pt: boolean; room: string; names: Record<string, string>; initialTab: ChannelEditTab;
  onClose: () => void; onError: (error: unknown, fallback: string) => void;
  /** The channel changed (name, topic): the list reloads. */
  onSaved: () => void;
}) {
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
      .catch(error => { if (active) { setFailed(true); reportError.current(error, pt ? "Não foi possível carregar o canal." : "Could not load the channel."); } });
    return () => { active = false; };
  }, [room, attempt, pt]);
  useEffect(() => { frame.current?.querySelector<HTMLButtonElement>("nav button")?.focus(); }, []);
  const save = async (change: Record<string, unknown>, done: string) => {
    if (saving) return;
    setSaving(true); setNotice("");
    try { load(await invoke<Settings>("matrix_channel_update", { room, change })); setNotice(done); onSaved(); }
    catch (error) { onError(error, pt ? "O Matrix não aceitou a mudança." : "Matrix did not accept the change."); }
    finally { setSaving(false); }
  };
  const mine = settings?.my_power ?? 0;
  const may = (need: number | undefined) => mine >= (need ?? ADMIN);
  // Three roles, as Discord shows them; another level set in Element stays listed as it is.
  const roleOptions = (current: number, base: number) => {
    const options = [{ value: String(base), label: pt ? "Todos os membros" : "All members" }, { value: String(MODERATOR), label: pt ? "Moderadores" : "Moderators" }, { value: String(ADMIN), label: pt ? "Administradores" : "Administrators" }];
    if (!options.some(option => Number(option.value) === current)) options.push({ value: String(current), label: pt ? `Nível ${current} (do Element)` : `Level ${current} (from Element)` });
    return options.map(option => ({ ...option, disabled: Number(option.value) > mine }));
  };
  const memberOptions = (current: number, base: number) => {
    const options = [{ value: String(base), label: pt ? "Membro" : "Member" }, { value: String(MODERATOR), label: pt ? "Moderador" : "Moderator" }, { value: String(ADMIN), label: pt ? "Administrador" : "Administrator" }];
    if (!options.some(option => Number(option.value) === current)) options.push({ value: String(current), label: pt ? `Nível ${current}` : `Level ${current}` });
    return options.map(option => ({ ...option, disabled: Number(option.value) >= mine && mine < 1_000_000 }));
  };
  const rows: [string, string, string][] = [
    ["send", pt ? "Enviar mensagens" : "Send messages", pt ? "Escrever no chat do canal." : "Write in the channel chat."],
    ["react", pt ? "Adicionar reações" : "Add reactions", ""],
    ["voice", pt ? "Conectar à voz" : "Connect to voice", pt ? "Entrar na chamada (e transmitir tela nela)." : "Join the call (and share screens in it)."],
    ["invite", pt ? "Convidar pessoas" : "Invite people", ""],
    ["pin", pt ? "Fixar mensagens" : "Pin messages", ""],
    ["redact", pt ? "Gerenciar mensagens" : "Manage messages", pt ? "Apagar mensagens dos outros." : "Remove other people's messages."],
    ["kick", pt ? "Expulsar membros" : "Kick members", ""],
    ["ban", pt ? "Banir membros" : "Ban members", ""],
    ["manage", pt ? "Gerenciar canal" : "Manage channel", pt ? "Nome, tópico e imagem." : "Name, topic and picture."],
    ["permissions", pt ? "Gerenciar permissões" : "Manage permissions", pt ? "Mudar esta lista e os cargos." : "Change this list and the roles."],
  ];
  const changedPermissions = settings ? Object.fromEntries(Object.entries(permissions).filter(([id, level]) => settings.permissions[id] !== level)) : {};
  const tabs: [ChannelEditTab, string][] = [["overview", pt ? "# visão geral" : "# overview"], ["permissions", pt ? "# permissões" : "# permissions"], ["roles", pt ? "# cargos" : "# roles"], ["invite", pt ? "# convidar" : "# invite"]];
  const canPermissions = may(settings?.needs.power_levels);
  return <div className="matrix-settings-overlay" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}
    onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); onClose(); } }}>
    <div ref={frame} className="matrix-settings matrix-channel-settings" role="dialog" aria-modal="true" aria-label={pt ? "Editar canal" : "Edit channel"}>
      <header><strong>{pt ? `> editar ${settings?.voice ? "🔊 " : "#"}${settings?.name || ""}` : `> edit ${settings?.voice ? "🔊 " : "#"}${settings?.name || ""}`}</strong><button type="button" className="matrix-settings-close" onClick={onClose} aria-label={pt ? "Fechar" : "Close"}>[x]</button></header>
      <nav role="tablist">{tabs.map(([id, label]) => <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setNotice(""); }}>{label}</button>)}</nav>
      <div className="matrix-settings-body" role="tabpanel">
        {failed ? <button type="button" onClick={() => setAttempt(value => value + 1)}>{pt ? "Tentar novamente" : "Try again"}</button> : !settings ? <p className="matrix-settings-hint">{pt ? "> lendo o canal no Matrix…" : "> reading the channel from Matrix…"}</p> : <>
          {tab === "overview" && <form className="matrix-channel-overview" onSubmit={event => {
            event.preventDefault();
            const change: Record<string, unknown> = {};
            if (name.trim() !== settings.name) change.name = name.trim();
            if (topic.trim() !== settings.topic) change.topic = topic.trim();
            if (joinRule !== settings.join_rule) change.join_rule = joinRule;
            if (Object.keys(change).length) void save(change, pt ? "> canal atualizado no Matrix" : "> channel updated on Matrix");
          }}>
            <div className="matrix-settings-field"><label htmlFor="matrix-channel-name">{pt ? "> nome do canal" : "> channel name"}</label>
              <input id="matrix-channel-name" value={name} maxLength={80} disabled={saving || !may(settings.needs.name)} onChange={event => setName(event.target.value)}/></div>
            <div className="matrix-settings-field"><label htmlFor="matrix-channel-topic">{pt ? "> tópico" : "> topic"}</label>
              <textarea id="matrix-channel-topic" value={topic} maxLength={1024} rows={3} disabled={saving || !may(settings.needs.topic)} onChange={event => setTopic(event.target.value)} placeholder={pt ? "Do que se fala aqui" : "What this channel is about"}/></div>
            <div className="matrix-settings-field"><span>{pt ? "> quem pode entrar" : "> who can join"}</span>
              <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? "Quem pode entrar" : "Who can join"} value={joinRule} disabled={saving || !may(settings.needs.join_rules)}
                onChange={setJoinRule} options={[
                  { value: "restricted", label: pt ? "Membros da comunidade" : "Community members" },
                  { value: "invite", label: pt ? "Só convidados (canal privado)" : "Invited only (private channel)" },
                  { value: "public", label: pt ? "Qualquer pessoa no Matrix" : "Anyone on Matrix" },
                  ...(["restricted", "invite", "public"].includes(settings.join_rule) ? [] : [{ value: settings.join_rule, label: settings.join_rule }]),
                ]}/></div>
            <small className="matrix-settings-hint">{pt ? "Muda o canal no Matrix: o Element e os outros aplicativos mostram o mesmo." : "Changes the channel on Matrix: Element and other apps show the same."}</small>
            <button type="submit" disabled={saving || (name.trim() === settings.name && topic.trim() === settings.topic && joinRule === settings.join_rule) || !name.trim()}>{saving ? (pt ? "[salvando…]" : "[saving…]") : (pt ? "[salvar]" : "[save]")}</button>
          </form>}
          {tab === "permissions" && <>
            <small className="matrix-settings-hint">{canPermissions ? (pt ? "Quem pode fazer cada coisa neste canal. Você só define até o seu próprio cargo." : "Who may do each thing in this channel. You can set up to your own role.") : (pt ? "Seu cargo pode ver, mas não mudar as permissões deste canal." : "Your role can view but not change this channel's permissions.")}</small>
            <div className="matrix-permission-list">
              {rows.filter(([id]) => id in permissions).map(([id, label, hint]) => <div key={id} className="matrix-permission-row">
                <span>{label}{hint && <small>{hint}</small>}</span>
                <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={label} value={String(permissions[id])} disabled={saving || !canPermissions || settings.permissions[id] > mine}
                  options={roleOptions(permissions[id], settings.users_default)} onChange={value => setPermissions(current => ({ ...current, [id]: Number(value) }))}/>
              </div>)}
            </div>
            {canPermissions && <button type="button" disabled={saving || !Object.keys(changedPermissions).length} onClick={() => void save({ permissions: changedPermissions }, pt ? "> permissões salvas no Matrix" : "> permissions saved on Matrix")}>{saving ? (pt ? "[salvando…]" : "[saving…]") : (pt ? "[salvar permissões]" : "[save permissions]")}</button>}
          </>}
          {tab === "roles" && <>
            <small className="matrix-settings-hint">{pt ? "Cargos neste canal. Criadores do canal têm poder total e não aparecem nos níveis." : "Roles in this channel. Channel creators have full power and are not listed by level."}</small>
            <ul className="matrix-role-list">
              {settings.creators.map(user => <li key={user}><span>{displayName(user, names)}<small>{user}</small></span><b>{pt ? "Criador" : "Creator"}</b></li>)}
              {settings.users.map(([user, level]) => <li key={user}><span>{displayName(user, names)}<small>{user}</small></span>
                <SelectionMenu className="matrix-select" menuClassName="matrix-select-menu" label={pt ? `Cargo de ${displayName(user, names)}` : `${displayName(user, names)}'s role`} value={String(level)} disabled={saving || !canPermissions || (level >= mine && mine < 1_000_000)}
                  options={memberOptions(level, settings.users_default)} onChange={value => void save({ users: { [user]: Number(value) === settings.users_default ? null : Number(value) } }, pt ? "> cargo atualizado" : "> role updated")}/>
              </li>)}
            </ul>
            {canPermissions && <form className="matrix-settings-field" onSubmit={event => { event.preventDefault(); const user = newRole.trim(); if (user) void save({ users: { [user]: MODERATOR } }, pt ? "> moderador adicionado" : "> moderator added").then(() => setNewRole("")); }}>
              <label htmlFor="matrix-role-user">{pt ? "> tornar moderador" : "> make moderator"}</label>
              <div><input id="matrix-role-user" value={newRole} maxLength={255} placeholder="@nome:matrix.org" disabled={saving} onChange={event => setNewRole(event.target.value)}/>
                <button type="submit" disabled={saving || !newRole.trim().startsWith("@")}>{pt ? "[adicionar]" : "[add]"}</button></div>
            </form>}
          </>}
          {tab === "invite" && <>
            <form className="matrix-settings-field" onSubmit={event => {
              event.preventDefault();
              const user = invitee.trim();
              if (!user || saving) return;
              setSaving(true); setNotice("");
              void invoke("matrix_channel_invite", { room, user }).then(() => { setNotice(pt ? `> convite enviado para ${user}` : `> invite sent to ${user}`); setInvitee(""); })
                .catch(error => onError(error, pt ? "Não foi possível convidar." : "Could not invite.")).finally(() => setSaving(false));
            }}>
              <label htmlFor="matrix-invite-user">{pt ? "> convidar uma conta Matrix" : "> invite a Matrix account"}</label>
              <div><input id="matrix-invite-user" value={invitee} maxLength={255} placeholder="@nome:matrix.org" disabled={saving} onChange={event => setInvitee(event.target.value)}/>
                <button type="submit" disabled={saving || !invitee.trim().startsWith("@")}>{saving ? (pt ? "[enviando…]" : "[sending…]") : (pt ? "[convidar]" : "[invite]")}</button></div>
              <small>{pt ? "A pessoa recebe o convite para o canal e para a comunidade." : "The person gets an invite to the channel and the community."}</small>
            </form>
            <div className="matrix-settings-field"><span>{pt ? "> link do canal" : "> channel link"}</span>
              <div><code>{channelLink(room)}</code><button type="button" onClick={() => void copyText(channelLink(room)).then(() => setNotice(pt ? "> link copiado" : "> link copied")).catch(error => onError(error, pt ? "Não foi possível copiar." : "Could not copy."))}>{pt ? "[copiar]" : "[copy]"}</button></div>
              <small>{pt ? "Abre o canal no Element ou em outro aplicativo Matrix. Quem não for da comunidade precisa de convite." : "Opens the channel in Element or another Matrix app. People outside the community need an invite."}</small>
            </div>
          </>}
        </>}
        {notice && <p className="matrix-settings-notice" role="status">{notice}</p>}
      </div>
    </div>
  </div>;
}
