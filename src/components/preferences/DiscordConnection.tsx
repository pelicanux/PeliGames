import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../MenuIcon";
import type { DiscordStatus as Status } from "../../theme/discordBenefits";
import { notifyError } from "../../services/notifications";
export function DiscordConnection() {
  const { language } = useI18n();
  const pt = language === "pt";
  const [status, setStatus] = useState<Status | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const active = useRef(false);
  const lock = useRef(false);
  const pendingSince = useRef(0);
  const load = async () => {
    const value = await invoke<Status>("load_discord_account");
    if (active.current) { setStatus(value); window.dispatchEvent(new CustomEvent("discordConnectionChanged", { detail: value })); }
  };
  useEffect(() => {
    active.current = true;
    const changed = (event: Event) => { if (active.current) setStatus((event as CustomEvent<Status>).detail); };
    window.addEventListener("discordConnectionChanged", changed);
    void load().catch(() => { if (active.current) setError(pt ? "Não foi possível consultar a conexão Discord." : "Could not check the Discord connection."); });
    return () => { active.current = false; window.removeEventListener("discordConnectionChanged", changed); };
  }, [pt]);
  useEffect(() => {
    if (!status?.pending) return;
    if (!pendingSince.current) pendingSince.current = Date.now();
    let stopped = false;
    let timer: ReturnType<typeof setTimeout>;
    const poll = async () => {
      if (stopped) return;
      if (Date.now() - pendingSince.current > 10 * 60 * 1000) {
        setStatus(previous => previous ? { ...previous, pending: false } : previous);
        setError(pt ? "O login expirou. Tente novamente." : "Sign-in expired. Try again.");
        return;
      }
      try { await load(); } catch { if (!stopped) setError(pt ? "Não foi possível verificar o login. Tente novamente." : "Could not verify sign-in. Try again."); }
      if (!stopped) timer = setTimeout(poll, 3000);
    };
    timer = setTimeout(poll, 3000);
    return () => { stopped = true; clearTimeout(timer); };
  }, [status?.pending, pt]);
  const run = async (action: "connect" | "disconnect" | "refresh") => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError("");
    try {
      if (action === "connect") {
        await invoke("connect_discord_account");
        pendingSince.current = Date.now();
        if (active.current) {
          const waiting = { configured: true, pending: true, offline: false, account: null };
          setStatus(waiting); window.dispatchEvent(new CustomEvent("discordConnectionChanged", { detail: waiting }));
        }
      } else {
        if (action === "disconnect") await invoke("disconnect_discord_account");
        await load();
      }
    } catch (e) {
      const message = typeof e === "string" ? e : pt ? "Não foi possível concluir a conexão." : "Could not complete the connection.";
      notifyError("Discord", message);
      if (active.current) setError(message);
    }
    finally { lock.current = false; if (active.current) setBusy(false); }
  };
  const account = status?.account;
  const valid = account?.benefit && account.valid_until && account.valid_until > Date.now();
  return <section className="preference-connection" data-preference-id="connection-discord" aria-busy={busy}>
    <header><MenuIcon name="discord"/><h3>Discord</h3><span className="preference-connection-status">{account ? pt ? "Conectado" : "Connected" : pt ? "Não conectado" : "Not connected"}</span></header>
    <p>{pt ? "Entre pelo navegador para verificar seu cargo de membro do YouTube ou seu impulso neste servidor." : "Sign in through your browser to check your YouTube membership role or your boost in this server."}</p>
    {!status && !error && <p role="status">{pt ? "Verificando conexão…" : "Checking connection…"}</p>}
    {status && !status.configured && <p>{pt ? "O serviço de login ainda precisa ser configurado para habilitar esta conexão." : "The sign-in service needs to be configured to enable this connection."}</p>}
    {status?.pending && <p role="status">{pt ? "Aguardando autorização no navegador. Depois de autorizar, você pode fechar a página." : "Waiting for browser authorization. You can close the page after authorizing."}</p>}
    {account && <>
      <p><strong>{account.name}</strong></p>
      <p>{!account.in_server ? pt ? "Entre no servidor da comunidade para verificar seus benefícios." : "Join the community server to check your benefits." : valid ? `${pt ? "Elegibilidade confirmada" : "Eligibility confirmed"}: ${account.benefit === "owner" ? pt ? "dono da comunidade" : "community owner" : account.benefit === "youtube" ? pt ? "membro do YouTube" : "YouTube member" : pt ? "impulsionador do servidor" : "server booster"}.` : pt ? "Nenhum benefício ativo confirmado. Verifique novamente após a sincronização dos cargos." : "No active benefit confirmed. Check again after roles have synced."}</p>
      {valid && <p>{pt ? "Validade da verificação" : "Verification valid until"}: {new Date(account.valid_until!).toLocaleDateString(pt ? "pt-BR" : "en-US")}</p>}
      {status?.offline && <p role="status">{pt ? "Sem validação online. Exibindo a última verificação salva." : "Online validation unavailable. Showing the last saved verification."}</p>}
    </>}
    <div className="nexus-account-actions">
      {!account && !status?.pending && <button type="button" className="btn btn-primary" disabled={busy || !status?.configured} onClick={() => void run("connect")}><MenuIcon name="discord"/>{pt ? "Entrar com Discord" : "Sign in with Discord"}</button>}
      {account && <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("refresh")}><MenuIcon name="repair"/>{pt ? "Verificar novamente" : "Check again"}</button>}
      {(account || status?.pending) && <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("disconnect")}><MenuIcon name="chainBroken"/>{status.pending ? pt ? "Cancelar login" : "Cancel sign-in" : pt ? "Desconectar" : "Disconnect"}</button>}
      {error && <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => void run("refresh")}><MenuIcon name="repair"/>{pt ? "Tentar novamente" : "Retry"}</button>}
    </div>
    {error && <p className="preferences-error" role="alert">{error}</p>}
  </section>;
}
