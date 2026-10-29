import { LogText } from "../../components/LogText";
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openBrowserUrl } from "../../services/browser";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "../nexus/NexusPopup";
import { useNexusText } from "../nexus/text";
import { MenuIcon } from "../../components/MenuIcon";
import { APP_BUILD_LABEL } from "../../services/buildInfo";
import { notifyError } from "../../services/notifications";
import { useI18n } from "../../i18n/I18nContext";
import type { FeatureReportContext } from "./reportContext";
interface Submission { title: string; description: string; discord: string; version: string; report: string }
export type ReportTarget = { kind: "nexus"; gameId: string } | { kind: "feature"; context: FeatureReportContext; label: string };
interface Report {path: string; content: string; bytes: number; truncated: boolean}
export function ReportModal({target,onClose}: {target: ReportTarget; onClose: () => void}) {
  const pt = useI18n().language === "pt";
  const [review, setReview] = useState<Submission>();
  const [preparing, setPreparing] = useState(false);
  const identity = JSON.stringify(target);
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
  useEffect(() => { void invoke<boolean>("report_relay_configured").then(setRelay).catch(()=>setRelay(false)); }, []);
  useEffect(() => {
    // Default title: the game named in the report.
    const game = report?.content.match(/^Jogo: (.*)$/m)?.[1]?.trim();
    if (game && !title) setTitle(`${game}${target.kind === "feature" ? ` · ${target.label}` : ""}: `);
  }, [report]); // eslint-disable-line react-hooks/exhaustive-deps
  const prepare = async () => {
    if (!report || preparing) return;
    setPreparing(true); setError("");
    try { setReview(await invoke<Submission>("prepare_report_submission", {title, description: details, discord, version: APP_BUILD_LABEL, report: report.content})); }
    catch (e) { setError(String(e)); }
    finally { setPreparing(false); }
  };
  const send = async () => {
    if (sending || !review) return;
    setSending(true);setError("");
    try { setSent(await invoke<string|null>("send_reviewed_report", {submission: review})); }
    catch (e) { setError(String(e)); notifyError(text.reportError, e); }
    finally { setSending(false); }
  };
  useEffect(() => {
    let active=true;setLoading(true);setError("");setReport(undefined);setCopied(false);setReview(undefined);setSent(undefined);
    void (target.kind === "nexus" ? invoke<Report>("export_nexus_report",{gameId:target.gameId,buildLabel:APP_BUILD_LABEL,description:""}) : invoke<Report>("export_feature_report",{context:target.context,buildLabel:APP_BUILD_LABEL})).then(value=>{if(active)setReport(value);}).catch(e=>{if(active)setError(String(e));}).finally(()=>{if(active)setLoading(false);});
    return ()=>{active=false;};
  },[identity,revision]);
  const open = (url: string) => {
    setError("");
    void openUrl(url).catch(() => openBrowserUrl(url)).catch(e=>setError(String(e)));
  };
  const copy = async () => {
    if (!report || copying) return;
    setCopying(true);setCopied(false);setError("");
    try {
      await invoke("plugin:clipboard-manager|write_text",{text:review?.report ?? report.content});
      setCopied(true);
    } catch (e) { setError(String(e)); }
    finally { setCopying(false); }
  };
  return <NexusPopup title={`${text.reportError}${target.kind === "feature" ? ` · ${target.label}` : ""}`} titleIcon={<MenuIcon name="bug" />} onClose={()=>{if(!sending)onClose();}} className="nexus-report-popup" footer={<>
    <button type="button" className="btn btn-secondary" disabled={!report || loading} onClick={()=>void invoke("open_folder",{path:report!.path.replace(/\/[^/]+$/,"")}).catch(e=>setError(String(e)))}><MenuIcon name="folder" />{text.reportFolder}</button>
    <button type="button" className="btn btn-primary" onClick={()=>open("https://discord.gg/78XSB9bHst")}><MenuIcon name="discord" />{text.reportInvite}</button>
  </>}>
    <p className="nexus-hint">{relay ? text.reportSendHint : text.reportHint}</p>
    <p className="nexus-hint">{pt ? "Inclui ajustes técnicos e logs desta instalação. Não anexa vídeos, áudio, capturas de tela ou credenciais. Revise o texto filtrado antes de confirmar." : "Includes technical settings and logs for this installation. No video, audio, screenshots or credentials are attached. Review the filtered text before confirming."}</p>
    {relay && <section className="nexus-game-checks">
      <label className="nexus-profile-name-field"><span>{text.reportTitleLabel}</span>
        <input type="text" className="nexus-profile-name-input" value={review?.title ?? title} maxLength={100} disabled={sending || preparing} readOnly={Boolean(review)} onChange={event=>setTitle(event.target.value)} /></label>
      <label className="nexus-profile-name-field"><span>{text.reportDetailsLabel}</span>
        <textarea className="nexus-profile-name-input" rows={3} value={review?.description ?? details} maxLength={1500} disabled={sending || preparing} readOnly={Boolean(review)} onChange={event=>setDetails(event.target.value)} /></label>
      <label className="nexus-profile-name-field"><span>{text.reportDiscordLabel}</span>
        <input type="text" className="nexus-profile-name-input" value={review?.discord ?? discord} maxLength={40} disabled={sending || preparing} readOnly={Boolean(review)} onChange={event=>setDiscord(event.target.value)} /></label>
      <p className="nexus-hint">{text.reportPublicNotice}</p>
      <div className="nexus-tabs">
        <button type="button" className="btn btn-primary" disabled={sending || preparing || loading || !report || !title.trim() || sent !== undefined} onClick={()=>void (review ? send() : prepare())}><MenuIcon name="discord" />{sending ? text.reportSending : preparing ? (pt ? "Preparando…" : "Preparing…") : review ? text.reportSend : (pt ? "Revisar envio" : "Review submission")}</button>
        {review && sent === undefined && <button type="button" className="btn btn-secondary" disabled={sending} onClick={()=>setReview(undefined)}>{pt ? "Voltar à edição" : "Back to editing"}</button>}
        {sent && <button type="button" className="btn btn-secondary" onClick={()=>open(sent)}><MenuIcon name="discord" />{text.reportOpenPost}</button>}
      </div>
      {sent !== undefined && <p className="nexus-hint" role="status">{text.reportSent}</p>}
    </section>}
    {error && <p className="nexus-error" role="alert">{error}</p>}
    {loading ? <p role="status">{text.loading}</p> : report && <>
      <div className="nexus-report-summary"><span>{report.path.split(/[\\/]/).pop()} · {(report.bytes/1024).toFixed(1)} KB{report.truncated ? (pt ? " · reduzido" : " · truncated") : ""}</span><div className="nexus-report-actions"><HoverTooltip text={copied ? text.reportCopied : text.reportCopy}><button type="button" className="btn btn-secondary nexus-report-copy" aria-label={text.reportCopy} disabled={copying} onClick={()=>void copy()}><MenuIcon name={copied ? "check" : "copy"} /></button></HoverTooltip><button type="button" className="btn btn-secondary" disabled={sending || preparing || Boolean(review)} onClick={()=>setRevision(v=>v+1)}><MenuIcon name="repair" />{text.reportRefresh}</button></div></div>
      {copied && <p className="nexus-hint" role="status">{text.reportCopied}</p>}
      <pre className="game-log-output nexus-report-preview" tabIndex={0}><LogText content={review?.report ?? report.content}/></pre>
    </>}
  </NexusPopup>;
}
