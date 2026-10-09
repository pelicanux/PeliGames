import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl, openNexusBrowser } from "../../services/browser";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
interface Account { user_id: number; name: string; is_premium: boolean; is_supporter: boolean }
export function NexusAccountPanel() {
  const text = useNexusText();
  const [account, setAccount] = useState<Account | null>(null);
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(true);
  const [error, setError] = useState("");
  const active = useRef(true);
  const lock = useRef(false);
  useEffect(() => {
    active.current = true;
    let cancelled = false;
    lock.current = true;
    void invoke<Account | null>("load_nexus_account").then(value => { if (!cancelled) setAccount(value); })
      .catch(() => { if (!cancelled) setError(text.accountLoadError); })
      .finally(() => { if (!cancelled) { lock.current = false; setBusy(false); } });
    return () => { cancelled = true; active.current = false; };
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
  const open = (url: string) => void openUrl(url).catch(() => setError(text.accountBrowserError));
  return <section className="nexus-account" aria-labelledby="nexus-account-title" aria-busy={busy}>
    <h3 id="nexus-account-title">{text.account}</h3>
    <p className="nexus-hint">{text.personalKeyHint}</p>
    {account && <div className="nexus-account-summary"><MenuIcon name="check" /><strong>{account.name}</strong><span className="nexus-status">{account.is_premium ? text.premiumAccount : text.freeAccount}</span></div>}
    {error && <p className="nexus-error" role="alert">{error}</p>}
    {busy && <p role="status" className="nexus-hint">{text.loading}</p>}
    {!account && <form className="nexus-fields" onSubmit={event => { event.preventDefault(); void run("connect"); }}>
      <label htmlFor="nexus-api-key">{text.personalKey}<input id="nexus-api-key" type="password" value={key} onChange={event => setKey(event.target.value)} autoComplete="off" spellCheck={false} autoCapitalize="none" maxLength={4096} disabled={busy} aria-describedby="nexus-key-security" /></label>
      <p id="nexus-key-security" className="nexus-hint">{text.keySecurity}</p>
      <button className="btn btn-primary" type="submit" disabled={busy || !key.trim()}><MenuIcon name="check" />{text.validateConnect}</button>
    </form>}
    <div className="nexus-account-actions">
      <button type="button" className="btn btn-secondary" onClick={() => void openNexusBrowser().catch(error => setError(typeof error === "string" ? error : error instanceof Error ? error.message : text.accountBrowserError))}><MenuIcon name="platform" />{text.internalNexusLogin}</button>
      {account && <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("verify")}><MenuIcon name="check" />{text.verifyAccount}</button>}
      <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("disconnect")}><MenuIcon name="trash" />{text.disconnectAccount}</button>
      <button type="button" className="btn btn-secondary" onClick={() => void openNexusBrowser("https://www.nexusmods.com/users/myaccount?tab=api").catch(error => setError(typeof error === "string" ? error : error instanceof Error ? error.message : text.accountBrowserError))}><MenuIcon name="platform" />{text.obtainKey}</button>
    </div>
    <p className="nexus-hint">{text.accountHint}</p>
    <p className="nexus-hint">{text.downloadAccountHint}</p>
    <button type="button" className="btn btn-secondary" onClick={() => open("https://www.nexusmods.com/")}><img className="nexus-icon" src="/nexus-mods.svg" alt="" />{text.website}</button>
  </section>;
}
