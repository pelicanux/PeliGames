import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { MenuIcon } from "../../components/MenuIcon";
import { HoverTooltip } from "../../components/HoverTooltip";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { useNexusText } from "./text";
import { NexusPopup } from "./NexusPopup";
import { NexusRequirements } from "./NexusRequirements";
import type { NexusGame } from "./useNexusWorkspace";
export interface GameCheck { kind: "unwanted_file" | "large_address" | "legacy_extender" | "missing_extender" | "missing_loader" | "missing_download" | "missing_nexus_download"; path: string; url: string | null; detail?: string; hidden: boolean; automatic?: boolean; recommended?: boolean }
export interface ExtenderError { dll: string; message: string; mod_id: string | null; mod_name: string | null; log: string }
export interface GameExtras { checks: GameCheck[]; extender_errors: ExtenderError[]; redirection: boolean | null }
/** Game folder checks (FNV Sanity Checks) and the BSA redirection switch (Vortex Archive Invalidation). */
export function NexusGameChecks({ game, extras, disabled, onChange, onDownloadLoader }: { game: NexusGame; extras: GameExtras; disabled: boolean; onChange: (extras: GameExtras) => void; onDownloadLoader?: (name?: string) => Promise<boolean> }) {
  const text = useNexusText();
  const gameId = game.id;
  const [download, setDownload] = useState<GameCheck | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const run = (command: string, args: Record<string, unknown>) => {
    if (working) return;
    setWorking(true); setError("");
    void invoke<GameExtras>(command, { gameId, ...args }).then(onChange).catch(e => setError(String(e))).finally(() => setWorking(false));
  };
  const message = (check: GameCheck) => check.kind === "unwanted_file" ? (check.hidden ? text.checkSetAside : text.checkUnwanted) : check.kind === "large_address" ? text.checkLargeAddress : check.kind === "missing_extender" ? `${text.checkMissingExtender} ${check.detail ?? ""}` : check.kind === "missing_loader" ? `${text.checkMissingLoader} ${check.detail ?? ""}` : check.kind === "missing_download" ? `${text.checkMissingDownload} ${check.detail ?? ""}${check.recommended ? ` · ${text.checkRecommended}` : ""}` : check.kind === "missing_nexus_download" ? `${text.checkMissingNexusDownload} ${check.detail ?? ""}${check.recommended ? ` · ${text.checkRecommended}` : ""}` : text.checkLegacyExtender;
  if (!extras.checks.length && !extras.extender_errors.length && extras.redirection === null) return null;
  return <section className="nexus-game-checks" aria-label={text.checksTitle}>
    {extras.checks.length > 0 && <><h4>{text.checksTitle}</h4>{extras.checks.map(check => <div key={`${check.kind}:${check.path}`} className={`nexus-game-check ${check.hidden ? "" : "nexus-requirements-warning"}`}>
      <span><strong>{check.path}</strong> · {check.kind === "unwanted_file" ? (check.hidden ? text.checkSetAsideShort : text.checkUnwantedShort) : message(check)}</span>
      {check.kind === "unwanted_file" && <HoverTooltip text={message(check)}><button type="button" className="installation-type-info" aria-label={message(check)}><MenuIcon name="info" /></button></HoverTooltip>}
      {check.kind === "unwanted_file" && <button type="button" className="btn btn-secondary" disabled={disabled || working} onClick={() => run("set_nexus_game_file_aside", { path: check.path, aside: !check.hidden })}><MenuIcon name={check.hidden ? "repair" : "folder"} />{check.hidden ? text.checkRestore : text.checkSetAsideAction}</button>}
      {check.kind === "missing_loader" && onDownloadLoader && <button type="button" className="btn btn-primary" disabled={disabled || working} onClick={() => { setWorking(true); void onDownloadLoader().finally(() => setWorking(false)); }}><MenuIcon name="download" />{working ? text.loading : text.checkDownloadLoader}</button>}
      {check.kind === "missing_download" && onDownloadLoader && <button type="button" className="btn btn-primary" disabled={disabled || working} onClick={() => { setWorking(true); void onDownloadLoader(check.path).finally(() => setWorking(false)); }}><MenuIcon name="download" />{working ? text.loading : `${text.checkDownloadNamed} ${check.path}`}</button>}
      {check.kind === "missing_nexus_download" && check.url && <button type="button" className="btn btn-primary" disabled={disabled} onClick={() => setDownload({ ...check, detail: check.path })}><MenuIcon name="download" />{`${text.checkDownloadNamed} ${check.path}`}</button>}
      {check.kind === "missing_extender" && check.url && <button type="button" className="btn btn-primary" disabled={disabled} onClick={() => setDownload(check)}><MenuIcon name="download" />{text.checkDownload}</button>}
      {check.url && <button type="button" className="btn btn-secondary" onClick={() => void openUrl(check.url!).catch(e => setError(String(e)))}><MenuIcon name="download" />{text.modPage}</button>}
    </div>)}</>}
    {extras.extender_errors.length > 0 && <><h4>{text.extenderErrors}</h4><p className="nexus-hint">{text.extenderErrorsHint}</p>
      {extras.extender_errors.map((item, index) => <div key={`${item.dll}:${index}`} className="nexus-game-check nexus-requirements-warning"><span><strong>{item.dll}</strong> · {item.mod_name ?? text.loadOrderExternal} · {item.message}</span></div>)}
      <div className="nexus-game-check">
        <button type="button" className="btn btn-secondary" onClick={() => void invoke("open_folder", { path: extras.extender_errors[0].log }).catch(e => setError(String(e)))}><MenuIcon name="logs" />{text.extenderLog}</button>
        <button type="button" className="btn btn-secondary" disabled={working} onClick={() => run("ignore_nexus_extender_errors", {})}><MenuIcon name="check" />{text.extenderIgnore}</button>
      </div></>}
    {download && <NexusPopup title={download.kind === "missing_extender" ? text.checkDownload : `${text.checkDownloadNamed} ${download.path}`} onClose={() => setDownload(null)}><p className="nexus-hint">{text.checkMissingExtenderHint} {download.detail}</p><NexusRequirements game={game} requirements={{ complete: true, items: [{ name: download.detail?.split(" · ")[0] ?? download.path, url: download.url!, kind: "nexus", notes: download.detail ?? "" }] }} onOpen={url => void openUrl(url).catch(e => setError(String(e)))} /></NexusPopup>}
    {extras.redirection !== null && <div className="nexus-load-order-auto">
      <label className="nexus-fomod-option"><input type="checkbox" checked={extras.redirection} disabled={disabled || working} onChange={event => run("set_nexus_bsa_redirection", { enabled: event.target.checked })} />{working ? text.loading : text.bsaRedirection}</label>
      <HoverTooltip text={text.bsaRedirectionHint}><button type="button" className="installation-type-info" aria-label={text.bsaRedirectionHint}><MenuIcon name="info" /></button></HoverTooltip>
    </div>}
    {error && <p role="alert" className="nexus-error">{error}</p>}
  </section>;
}
