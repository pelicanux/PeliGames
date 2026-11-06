import { ConnectionStorageChoice } from "../../components/ConnectionStorageChoice";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl, openNexusBrowser } from "../../services/browser";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
import { useI18n } from "../../i18n/I18nContext";
interface Account { user_id: number; name: string; is_premium: boolean; is_supporter: boolean }
export function NexusAccountPanel() {
  const {t: tx} = useI18n();

  const text = useNexusText();
  const [oauth, setOauth] = useState({ configured: false, pending: false });
  const [authorizing, setAuthorizing] = useState(false);
  const [account, setAccount] = useState<Account | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const active = useRef(true);
  const lock = useRef(false);
  useEffect(() => {
    active.current = true;
    let cancelled = false;
    const status = () => void invoke<{ configured: boolean; pending: boolean }>("nexus_oauth_status").then(value => { if (!cancelled) setOauth(value); }).catch(() => {});
    status();
    const interval = window.setInterval(status, 1000);
    lock.current = true;
    void invoke<Account | null>("load_nexus_account").then(value => { if (!cancelled) setAccount(value); })
      .catch(() => { if (!cancelled) setError(text.accountLoadError); })
      .finally(() => { if (!cancelled) { lock.current = false; setBusy(false); } });
    return () => { window.clearInterval(interval); cancelled = true; active.current = false; };
  }, [text.accountLoadError]);
  const run = async (operation: "connect" | "disconnect" | "verify") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    const apiKey = key.trim();
    // Clear the input as soon as it is submitted; never persist it in browser storage.
    if (operation === "connect") setKey("");
    try {
      if (operation === "disconnect") {
        await invoke("disconnect_nexus_account");
        if (active.current) { setAccount(null); setKey(""); }
      } else {
        const value = await invoke<Account | null>(operation === "connect" ? "connect_nexus_account" : "load_nexus_account", operation === "connect" ? { apiKey } : undefined);
        if (active.current) setAccount(value);
      }
    } catch (e) {
      // Backend messages are sanitized. Avoid echoing arguments or bridge errors.
      if (active.current) setError(typeof e === "string" && !e.includes(apiKey || "\0") ? e : text.accountOperationError);
    } finally { lock.current = false; if (active.current) setBusy(false); }
  };
  const login = async () => {
    if (lock.current || oauth.pending) return;
    lock.current = true; setBusy(true); setAuthorizing(true); setError(""); setKey("");
    try {
      const value = await invoke<Account>("connect_nexus_oauth");
      if (active.current) setAccount(value);
    } catch (e) {
      if (active.current) setError(typeof e === "string" ? e : text.accountOperationError);
    } finally { lock.current = false; if (active.current) { setBusy(false); setAuthorizing(false); } }
  };
  const open = (url: string) => void openUrl(url).catch(() => setError(text.accountBrowserError));
  return <div className="connection-grid" aria-busy={busy}>
    <section className="connection-card"><header className="connection-heading"><span className="mod-component-icon"><img src="/nexus-mods.svg" alt=""/></span><div><h3>Nexus Mods</h3><p>{tx("ui-nexus-account-panel", "connect_your_account_to_access_online_mods")}</p></div><span className="mod-status-badge" data-ready={Boolean(account)} role="status">{account?(tx("ui-nexus-account-panel", "connected")):authorizing||oauth.pending?(tx("ui-nexus-account-panel", "waiting_for_sign_in")):busy?text.loading:(tx("ui-nexus-account-panel", "not_connected"))}</span></header>
    <ConnectionStorageChoice service="nexus" disabled={Boolean(account)||authorizing||oauth.pending} onChange={()=>{setError("");void run("verify");}}/>
    {account?<div className="connection-account-row"><MenuIcon name="check"/><div><strong>{account.name}</strong><p>{account.is_premium?text.premiumAccount:text.freeAccount}</p></div></div>:<><p className="connection-section-hint">{tx("ui-nexus-account-panel", "authorize_in_your_browser_or_use_your_personal_api_key")}</p><div className="connection-primary-actions"><button type="button" className="mod-action-primary" disabled={busy||oauth.pending||!oauth.configured} onClick={()=>void login()}><MenuIcon name="chainLinked"/>{text.oauthLogin}</button>{(authorizing||oauth.pending)&&<button type="button" className="mod-action-secondary" onClick={()=>void invoke("cancel_nexus_oauth").catch(()=>setError(text.accountOperationError))}>{text.oauthCancel}</button>}</div></>}
    {(authorizing||oauth.pending)&&<p role="status">{text.oauthWaiting}</p>}
    {!account&&!oauth.configured&&<p className="connection-section-hint">{text.oauthUnavailable}</p>}
    {!account&&<details className="connection-api-details" open={!oauth.configured}><summary>{tx("ui-nexus-account-panel", "connect_with_a_personal_key")}</summary><form className="connection-key-form" onSubmit={event=>{event.preventDefault();void run("connect")}}><label htmlFor="nexus-preferences-api-key">{text.personalKey}<input id="nexus-preferences-api-key" type="password" value={key} onChange={event=>setKey(event.target.value)} autoComplete="off" spellCheck={false} autoCapitalize="none" maxLength={4096} disabled={busy} aria-describedby="nexus-preferences-key-security"/></label><small id="nexus-preferences-key-security">{text.keySecurity}</small><div className="connection-primary-actions"><button type="submit" className="mod-action-primary" disabled={busy||!key.trim()}><MenuIcon name="check"/>{text.validateConnect}</button><button type="button" className="mod-action-secondary" onClick={()=>void openNexusBrowser("https://www.nexusmods.com/users/myaccount?tab=api").catch(error=>setError(typeof error==="string"?error:error instanceof Error?error.message:text.accountBrowserError))}>{text.obtainKey}</button></div></form></details>}
    {account&&<div className="connection-primary-actions"><button type="button" className="mod-action-secondary" disabled={busy} onClick={()=>void run("verify")}><MenuIcon name="repair"/>{text.verifyAccount}</button></div>}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    <div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{tx("ui-nexus-account-panel", "remove_connection")}</strong><small>{tx("ui-nexus-account-panel", "deletes_credentials_from_this_device_your_account_and_mods_are_preserved")}</small></div><button type="button" className="mod-action-quiet danger" disabled={busy||oauth.pending} onClick={()=>void run("disconnect")}><MenuIcon name="chainBroken"/>{tx("ui-nexus-account-panel", "disconnect")}</button></div>
    </section>
    <section className="connection-card"><h3>{tx("ui-nexus-account-panel", "access_to_mods")}</h3><p className="connection-section-hint">{tx("ui-nexus-account-panel", "after_connecting_open_a_game_in_the_library_and_go_to_mods")}</p><div className="connection-info-row"><MenuIcon name="puzzle"/><div><strong>{tx("ui-nexus-account-panel", "nexus_downloads")}</strong><p>{text.downloadAccountHint}</p></div></div><div className="connection-info-row"><MenuIcon name="folder"/><div><strong>{tx("ui-nexus-account-panel", "local_mods_remain_available")}</strong><p>{tx("ui-nexus-account-panel", "you_can_install_your_own_files_without_connecting_an_account")}</p></div></div><div className="mod-setting-row"><div className="mod-row-copy"><strong>{tx("ui-nexus-account-panel", "internal_browser")}</strong><small>{tx("ui-nexus-account-panel", "sign_in_to_the_website_and_manage_your_nexus_account")}</small></div><button type="button" className="mod-action-secondary" onClick={()=>void openNexusBrowser().catch(error=>setError(typeof error==="string"?error:error instanceof Error?error.message:text.accountBrowserError))}><MenuIcon name="platform"/>{tx("ui-nexus-account-panel", "open")}</button></div><div className="connection-primary-actions"><button type="button" className="mod-action-quiet" onClick={()=>open("https://www.nexusmods.com/")}><MenuIcon name="externalLink"/>{text.website}</button></div></section>
  </div>;
}
