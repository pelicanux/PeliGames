import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { openBrowserUrl } from "../../services/browser";
import { HoverTooltip } from "../../components/HoverTooltip";
import { NexusPopup } from "./NexusPopup";
import { useNexusText } from "./text";
import { MenuIcon } from "../../components/MenuIcon";
import { APP_BUILD_LABEL } from "../../services/buildInfo";
interface Report {path: string; content: string; bytes: number; truncated: boolean}
export function NexusReportModal({gameId,onClose}: {gameId: string; onClose: () => void}) {
  const text=useNexusText();
  const [report,setReport]=useState<Report>();
  const [error,setError]=useState("");
  const [copied,setCopied]=useState(false);
  const [copying,setCopying]=useState(false);
  const [revision,setRevision]=useState(0);
  const [loading,setLoading]=useState(true);
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
    <button type="button" className="btn btn-secondary" onClick={()=>open("https://discord.gg/78XSB9bHst")}><MenuIcon name="discord" />{text.reportInvite}</button>
    <button type="button" className="btn btn-primary" onClick={()=>open("https://discord.com/channels/1088170837921779812/1557974055531847700")}><MenuIcon name="discord" />{text.reportChannel}</button>
  </>}>
    <p className="nexus-hint">{text.reportHint}</p>
    {error && <p className="nexus-error" role="alert">{error}</p>}
    {loading ? <p role="status">{text.loading}</p> : report && <>
      <div className="nexus-report-summary"><span>relatorio-nexus.txt · {(report.bytes/1024).toFixed(1)} KB</span><div className="nexus-report-actions"><HoverTooltip text={copied ? text.reportCopied : text.reportCopy}><button type="button" className="btn btn-secondary nexus-report-copy" aria-label={text.reportCopy} disabled={copying} onClick={()=>void copy()}><MenuIcon name={copied ? "check" : "copy"} /></button></HoverTooltip><button type="button" className="btn btn-secondary" onClick={()=>setRevision(v=>v+1)}><MenuIcon name="repair" />{text.reportRefresh}</button></div></div>
      {copied && <p className="nexus-hint" role="status">{text.reportCopied}</p>}
      <pre className="game-log-output nexus-report-preview" tabIndex={0}>{report.content}</pre>
    </>}
  </NexusPopup>;
}
