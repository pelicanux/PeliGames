import { ConnectionStorageChoice } from "../ConnectionStorageChoice";
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
    void load().catch(() => { if (active.current) setError(pt ? "Não foi possível consultar a conexão Discord." : "Could not check the Discord connection."); });
    return () => { active.current = false; revision.current++; window.removeEventListener("discordConnectionChanged", changed); };
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
      const message = typeof e === "string" ? e : pt ? "Não foi possível concluir a conexão." : "Could not complete the connection.";
      notifyError("Discord", message);
      if (active.current) setError(message);
    }
    finally { mutating.current = false; lock.current = false; if (active.current) setBusy(false); }
  };
  const account = status?.account;
  const valid = account?.benefit && account.valid_until && account.valid_until > Date.now();
  return <section className="connection-module" data-preference-id="connection-discord" aria-busy={busy}><div className="connection-grid">
    <section className="connection-card"><header className="connection-heading"><span className="mod-component-icon"><MenuIcon name="discord"/></span><div><h3>Discord</h3><p>{pt?"Vincule sua conta à comunidade.":"Link your account to the community."}</p></div><span className="mod-status-badge" data-ready={Boolean(account)} role="status">{account?(pt?"Conectado":"Connected"):status?.pending?(pt?"Aguardando login":"Waiting for sign-in"):!status&&!error?(pt?"Verificando…":"Checking…"):(pt?"Não conectado":"Not connected")}</span></header>
    <ConnectionStorageChoice service="discord" disabled={busy||Boolean(account)||Boolean(status?.pending)} onChange={()=>void run("refresh")}/>
    <div className="connection-account-row"><MenuIcon name={account?"check":"chainLinked"}/><div><strong>{account?account.name:(pt?"Autorize pelo navegador":"Authorize in your browser")}</strong><p>{pt?"O PeliGames verifica os cargos vinculados à sua conta.":"PeliGames checks the roles linked to your account."}</p></div></div>
    {status&&!status.configured&&<p role="status">{pt?"O serviço de login ainda precisa ser configurado para habilitar esta conexão.":"The sign-in service needs to be configured to enable this connection."}</p>}
    {status?.pending&&<p role="status">{pt?"Aguardando autorização no navegador. Depois de autorizar, você pode fechar a página.":"Waiting for browser authorization. You can close the page after authorizing."}</p>}
    <div className="connection-primary-actions">
      {!account&&!status?.pending&&<button type="button" className="mod-action-primary" disabled={busy||!status?.configured} onClick={()=>void run("connect")}><MenuIcon name="discord"/>{pt?"Entrar com Discord":"Sign in with Discord"}</button>}
      {account&&<button type="button" className="mod-action-secondary" disabled={busy} onClick={()=>void run("refresh")}><MenuIcon name="repair"/>{pt?"Verificar novamente":"Check again"}</button>}
      {error&&<button type="button" className="mod-action-secondary" disabled={busy} onClick={()=>void run("refresh")}><MenuIcon name="repair"/>{pt?"Tentar novamente":"Retry"}</button>}
    </div>
    {(account||status?.pending)&&<div className="mod-setting-row mod-destructive-row"><div className="mod-row-copy"><strong>{status?.pending?(pt?"Cancelar autorização":"Cancel authorization"):(pt?"Remover conexão":"Remove connection")}</strong><small>{pt?"Remove a conexão deste dispositivo, sem excluir sua conta Discord.":"Removes the connection from this device without deleting your Discord account."}</small></div><button type="button" className="mod-action-quiet danger" disabled={busy} onClick={()=>void run("disconnect")}><MenuIcon name="chainBroken"/>{status?.pending?(pt?"Cancelar":"Cancel"):(pt?"Desconectar":"Disconnect")}</button></div>}
    {error&&<p className="mod-feedback-error" role="alert">{error}</p>}
    </section>
    <section className="connection-card"><h3>{pt?"Verificação de benefícios":"Benefit verification"}</h3><p className="connection-section-hint">{pt?"O cargo de membro do YouTube ou o impulso no servidor pode liberar benefícios no launcher.":"A YouTube membership role or a server boost may unlock launcher benefits."}</p>
      <div className="connection-info-row"><MenuIcon name="heart"/><div><strong>{account?(valid?(pt?"Elegibilidade confirmada":"Eligibility confirmed"):(pt?"Sem benefício confirmado":"No confirmed benefit")):(pt?"Conecte para verificar":"Connect to check")}</strong><p>{account?(!account.in_server?(pt?"Entre no servidor da comunidade para verificar seus benefícios.":"Join the community server to check your benefits."):valid?(account.benefit==="owner"?(pt?"Dono da comunidade":"Community owner"):account.benefit==="youtube"?(pt?"Membro do YouTube":"YouTube member"):(pt?"Impulsionador do servidor":"Server booster")):(pt?"Se seus cargos mudaram, use Verificar novamente após a sincronização.":"If your roles have changed, use Check again after synchronization.")):(pt?"Entre com a conta que tem seus cargos no servidor.":"Sign in with the account holding your server roles.")}</p></div></div>
      {valid&&<div className="connection-info-row"><MenuIcon name="calendar"/><div><strong>{pt?"Validade da verificação":"Verification valid until"}</strong><p>{new Date(account.valid_until!).toLocaleDateString(pt?"pt-BR":"en-US")}</p></div></div>}
      {status?.offline&&<p role="status">{pt?"Sem validação online. Exibindo a última verificação salva.":"Online validation unavailable. Showing the last saved verification."}</p>}
    </section>
  </div></section>;
}
