import {DiscordPresencePreferences} from "./DiscordPresencePreferences";
import { ConnectionStorageChoice } from "../ConnectionStorageChoice";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import type { DiscordStatus as Status } from "../../theme/discordBenefits";
import { notifyError } from "../../services/notifications";
export function DiscordConnection() {
  const {t: tx, language} = useI18n();

  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const active = useRef(false);
  const lock = useRef(false);
  const revision = useRef(0);
  const mutating = useRef(false);
  const pendingSince = useRef(0);
  const load = async () => {
    if (mutating.current) return;
    const request = ++revision.current;
    const value = await invoke<Status>("load_discord_account");
    if (active.current && request === revision.current && !mutating.current) { setStatus(value); window.dispatchEvent(new CustomEvent("discordConnectionChanged", { detail: value })); }
  };
  useEffect(() => {
    active.current = true;
    const changed = (event: Event) => { revision.current++; if (active.current) setStatus((event as CustomEvent<Status>).detail); };
    window.addEventListener("discordConnectionChanged", changed);
    void load().catch(() => { if (active.current) setError(tx("ui-discord-connection", "could_not_check_the_discord_connection")); });
    return () => { active.current = false; revision.current++; window.removeEventListener("discordConnectionChanged", changed); };
  }, [tx]);
  useEffect(() => {
    if (!status?.pending) return;
    if (!pendingSince.current) pendingSince.current = Date.now();
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (stopped) return;
      if (Date.now() - pendingSince.current > 10 * 60 * 1000) {
        setStatus(previous => previous ? { ...previous, pending: false } : previous);
        setError(tx("ui-discord-connection", "sign_in_expired_try_again"));
        return;
      }
      try { await load(); } catch { if (!stopped) setError(tx("ui-discord-connection", "could_not_verify_sign_in_try_again")); }
      if (!stopped) timer = setTimeout(poll, 3000);
    };
    timer = setTimeout(poll, 3000);
    return () => { stopped = true; clearTimeout(timer); };
  }, [status?.pending, tx]);
  const run = async (action: "connect" | "disconnect" | "refresh") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    mutating.current = action !== "refresh";
    revision.current++;
    try {
      if (action === "connect") {
        await invoke("connect_discord_account");
        pendingSince.current = Date.now();
        if (active.current) {
          const waiting = { configured: true, pending: true, offline: false, account: null };
          setStatus(waiting); window.dispatchEvent(new CustomEvent("discordConnectionChanged", { detail: waiting }));
        }
      } else {
        if (action === "disconnect") {
          await invoke("disconnect_discord_account");
          pendingSince.current = 0;
          const disconnected: Status = { configured: status?.configured ?? true, pending: false, offline: false, account: null, session_id: null, entitlement: null };
          if (active.current) setStatus(disconnected);
          window.dispatchEvent(new CustomEvent("discordConnectionChanged", { detail: disconnected }));
        } else await load();
      }
    } catch (e) {
      const message = typeof e === "string" ? e : tx("ui-discord-connection", "could_not_complete_the_connection");
      notifyError("Discord", message);
      if (active.current) setError(message);
    }
    finally { mutating.current = false; lock.current = false; if (active.current) setBusy(false); }
  };
  const account = status?.account;
  const valid = account?.benefit && account.valid_until && account.valid_until > Date.now();
  return <section className="connection-module" data-preference-id="connection-discord" aria-busy={busy}><div className="connection-grid">
    <section className="connection-card"><header className="connection-heading"><span className="mod-component-icon"><MenuIcon name="discord"/></span><div><h3>Discord</h3><p>{tx("ui-discord-connection", "link_your_account_to_the_community")}</p></div><span className="mod-status-badge" data-ready={Boolean(account)} role="status">{account?(tx("ui-discord-connection", "connected")):status?.pending?(tx("ui-discord-connection", "waiting_for_sign_in")):!status&&!error?(tx("ui-discord-connection", "checking")):(tx("ui-discord-connection", "not_connected"))}</span></header>
    <ConnectionStorageChoice service="discord" disabled={busy||Boolean(account)||Boolean(status?.pending)} onChange={()=>void run("refresh")}/>
    <div className="connection-account-row"><MenuIcon name={account?"check":"chainLinked"}/><div><strong>{account?account.name:(tx("ui-discord-connection", "authorize_in_your_browser"))}</strong><p>{tx("ui-discord-connection", "peligames_checks_the_roles_linked_to_your_account")}</p></div></div>
    {status&&!status.configured&&<p role="status">{tx("ui-discord-connection", "the_sign_in_service_needs_to_be_configured_to_enable_this_connection")}</p>}
    {status?.pending&&<p role="status">{tx("ui-discord-connection", "waiting_for_browser_authorization_you_can_close_the_page_after_authorizing")}</p>}
    <div className="connection-primary-actions">
      {!account&&!status?.pending&&<button type="button" className="mod-action-primary" disabled={busy||!status?.configured} onClick={()=>void run("connect")}><MenuIcon name="discord"/>{tx("ui-discord-connection", "sign_in_with_discord")}</button>}
      {account&&<button type="button" className="mod-action-secondary" disabled={busy} onClick={()=>void run("refresh")}><MenuIcon name="repair"/>{tx("ui-discord-connection", "check_again")}</button>}
      {error&&<button type="button" className="mod-action-secondary" disabled={busy} onClick={()=>void run("refresh")}><MenuIcon name="repair"/>{tx("ui-discord-connection", "retry")}</button>}
    </div>
    {(account||status?.pending)&&<div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{status?.pending?(tx("ui-discord-connection", "cancel_authorization")):(tx("ui-discord-connection", "remove_connection"))}</strong><small>{tx("ui-discord-connection", "removes_the_connection_from_this_device_without_deleting_your_discord_account")}</small></div><button type="button" className="mod-action-quiet danger" disabled={busy} onClick={()=>void run("disconnect")}><MenuIcon name="chainBroken"/>{status?.pending?(tx("ui-discord-connection", "cancel")):(tx("ui-discord-connection", "disconnect"))}</button></div>}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    </section>
    <section className="connection-card"><h3>{tx("ui-discord-connection", "benefit_verification")}</h3><p className="connection-section-hint">{tx("ui-discord-connection", "a_youtube_membership_role_or_a_server_boost_may_unlock_launcher_benefits")}</p>
      <div className="connection-info-row"><MenuIcon name="heart"/><div><strong>{account?(valid?(tx("ui-discord-connection", "eligibility_confirmed")):(tx("ui-discord-connection", "no_confirmed_benefit"))):(tx("ui-discord-connection", "connect_to_check"))}</strong><p>{account?(!account.in_server?(tx("ui-discord-connection", "join_the_community_server_to_check_your_benefits")):valid?(account.benefit==="owner"?(tx("ui-discord-connection", "community_owner")):account.benefit==="youtube"?(tx("ui-discord-connection", "youtube_member")):(tx("ui-discord-connection", "server_booster"))):(tx("ui-discord-connection", "if_your_roles_have_changed_use_check_again_after_synchronization"))):(tx("ui-discord-connection", "sign_in_with_the_account_holding_your_server_roles"))}</p></div></div>
      {valid&&<div className="connection-info-row"><MenuIcon name="calendar"/><div><strong>{tx("ui-discord-connection", "verification_valid_until")}</strong><p>{new Date(account.valid_until!).toLocaleDateString(language)}</p></div></div>}
      {status?.offline&&<p role="status">{tx("ui-discord-connection", "online_validation_unavailable_showing_the_last_saved_verification")}</p>}
    </section>
    <DiscordPresencePreferences/>
  </div></section>;
}
