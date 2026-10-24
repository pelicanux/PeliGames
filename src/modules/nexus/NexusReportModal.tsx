import { LogText } from "../../components/LogText";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openBrowserUrl } from "../../services/browser";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import { MenuIcon } from "../../components/MenuIcon";
import { APP_BUILD_LABEL } from "../../services/buildInfo";
import { notifyError } from "../../services/notifications";
interface Report {path: string; content: string; bytes: number; truncated: boolean}
export function NexusReportModal({gameId,onClose}: {gameId: string; onClose: () => void}) {
  const text=useNexusText();
  const [report,setReport]=useState<Report>();
  const [error,setError]=useState("");
  const [copied,setCopied]=useState(false);
  const [copying,setCopying]=useState(false);
  const [revision,setRevision]=useState(0);
  const [loading,setLoading]=useState(true);
  // Direct send to the Discord report forum, through the bridge (tools/report-relay).
  const [relay,setRelay]=useState(false);
  const [title,setTitle]=useState("");
  const [details,setDetails]=useState("");
  const [discord,setDiscord]=useState("");
  const [sending,setSending]=useState(false);
  const [sent,setSent]=useState<string|null|undefined>(undefined);
  useEffect(() => { void invoke<boolean>("nexus_report_relay_configured").then(setRelay).catch(()=>setRelay(false)); }, []);
  useEffect(() => {
    // Default title: the game named in the report.
    const game = report?.content.match(/^Jogo: (.*)$/m)?.[1]?.trim();
    if (game && !title) setTitle(`${game}: `);
  }, [report]); // eslint-disable-line react-hooks/exhaustive-deps
  const send = async () => {
    if (sending || !title.trim()) return;
    setSending(true);setError("");setSent(undefined);
    try { setSent(await invoke<string|null>("send_nexus_report",{gameId,buildLabel:APP_BUILD_LABEL,title:title.trim(),description:details.trim(),discord:discord.trim()})); }
    catch (e) { setError(String(e)); notifyError(text.reportError, e); }
    finally { setSending(false); }
  };
  useEffect(() => {
    let active=true;setLoading(true);setError("");setReport(undefined);setCopied(false);
    void invoke<Report>("export_nexus_report",{gameId,buildLabel:APP_BUILD_LABEL,description:""}).then(value=>{if(active)setReport(value);}).catch(e=>{if(active)setError(String(e));}).finally(()=>{if(active)setLoading(false);});
    return ()=>{active=false;};
  },[gameId,revision]);
  const open = (url: string) => {
    setError("");
    void openUrl(url).catch(() => openBrowserUrl(url)).catch(e=>setError(String(e)));
  };
  const copy = async () => {
    if (!report || copying) return;
    setCopying(true);setCopied(false);setError("");
    try {
      await invoke("plugin:clipboard-manager|write_text",{text:report.content});
      setCopied(true);
    } catch (e) { setError(String(e)); }
    finally { setCopying(false); }
  };
  return <NexusPopup title={text.reportError} onClose={onClose} className="nexus-report-popup" footer={<>
    <button type="button" className="btn btn-secondary" disabled={!report || loading} onClick={()=>void invoke("open_folder",{path:report!.path.replace(/\/[^/]+$/,"")}).catch(e=>setError(String(e)))}><MenuIcon name="folder" />{text.reportFolder}</button>
    <button type="button" className="btn btn-primary" onClick={()=>open("https://discord.gg/78XSB9bHst")}><MenuIcon name="discord" />{text.reportInvite}</button>
  </>}>
    <p className="nexus-hint">{relay ? text.reportSendHint : text.reportHint}</p>
    {relay && <section className="nexus-game-checks">
      <label className="nexus-profile-name-field"><span>{text.reportTitleLabel}</span>
        <input type="text" className="nexus-profile-name-input" value={title} maxLength={100} disabled={sending} onChange={event=>setTitle(event.target.value)} /></label>
      <label className="nexus-profile-name-field"><span>{text.reportDetailsLabel}</span>
        <textarea className="nexus-profile-name-input" rows={3} value={details} maxLength={1500} disabled={sending} onChange={event=>setDetails(event.target.value)} /></label>
      <label className="nexus-profile-name-field"><span>{text.reportDiscordLabel}</span>
        <input type="text" className="nexus-profile-name-input" value={discord} maxLength={40} disabled={sending} onChange={event=>setDiscord(event.target.value)} /></label>
      <p className="nexus-hint">{text.reportPublicNotice}</p>
      <div className="nexus-tabs">
        <button type="button" className="btn btn-primary" disabled={sending || loading || !report || !title.trim() || sent !== undefined} onClick={()=>void send()}><MenuIcon name="discord" />{sending ? text.reportSending : text.reportSend}</button>
        {sent && <button type="button" className="btn btn-secondary" onClick={()=>open(sent)}><MenuIcon name="discord" />{text.reportOpenPost}</button>}
      </div>
      {sent !== undefined && <p className="nexus-hint" role="status">{text.reportSent}</p>}
    </section>}
    {error && <p className="nexus-error" role="alert">{error}</p>}
    {loading ? <p role="status">{text.loading}</p> : report && <>
      <div className="nexus-report-summary"><span>relatorio-nexus.txt · {(report.bytes/1024).toFixed(1)} KB</span><div className="nexus-report-actions"><HoverTooltip text={copied ? text.reportCopied : text.reportCopy}><button type="button" className="btn btn-secondary nexus-report-copy" aria-label={text.reportCopy} disabled={copying} onClick={()=>void copy()}><MenuIcon name={copied ? "check" : "copy"} /></button></HoverTooltip><button type="button" className="btn btn-secondary" onClick={()=>setRevision(v=>v+1)}><MenuIcon name="repair" />{text.reportRefresh}</button></div></div>
      {copied && <p className="nexus-hint" role="status">{text.reportCopied}</p>}
      <pre className="game-log-output nexus-report-preview" tabIndex={0}><LogText content={report.content}/></pre>
    </>}
  </NexusPopup>;
}
