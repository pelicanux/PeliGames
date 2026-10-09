import { groupNexusFiles } from "./fileGroups";
import { NexusDownloadQueue } from "./NexusDownloadQueue";
import { NexusPageSizeSelector } from "./NexusPageSizeSelector";
import { nexusArchiveFormats } from "./archiveFormats";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openBrowserUrl as openUrl } from "../../services/browser";
import { openFilePicker } from "../../services/tauriService";
import { HoverTooltip } from "../../components/HoverTooltip";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
import { NexusPopup } from "./NexusPopup";
import { NexusRequirements, plainNexusText, type Requirements } from "./NexusRequirements";
import type { NexusGame } from "./useNexusWorkspace";
import { matchingCatalogGame, filterCatalogGames, type CatalogGame } from "./catalogGameMatch";
interface Mod { mod_id: number; name: string; summary: string; description?: string; picture_url?: string | null; author: string; version: string; downloads?: number | null }
interface CatalogPage { mods: Mod[]; total_count: number; next_offset: number }
interface File { file_id: number; name: string; file_name: string; version: string; size: number; category_name?: string; category_id?: number; description?: string }
interface Details { info: Mod; files: File[]; requirements: Requirements }
const plain = plainNexusText;
function ModThumbnail({ mod }: { mod: Mod }) {
  const text = useNexusText();
  const [failedUrl, setFailedUrl] = useState<string | null>(null);
  let url = "";
  try {
    const image = new URL(mod.picture_url || "");
    if (image.protocol === "https:" && !image.username && !image.password) url = image.href;
  } catch { /* Missing or invalid images use the same placeholder. */ }
  return <div className="nexus-mod-thumbnail">
    {url && failedUrl !== url
      ? <img src={url} alt={mod.name} loading="lazy" decoding="async" referrerPolicy="no-referrer" onError={() => setFailedUrl(url)} />
      : <div className="nexus-thumbnail-placeholder"><MenuIcon name="image" /><span>{text.noThumbnail}</span></div>}
  </div>;
}
export function NexusCatalogPanel({ game, onAssociate, onImport, onClose }: {
  game?: NexusGame; onAssociate: (id: string, domain: string) => Promise<boolean>;
  onImport: (id: string, source: string) => Promise<boolean>; onClose: () => void;
}) {
  const text = useNexusText();
  const identifiedDomain = game?.adapter || "";
  const domain = identifiedDomain || game?.nexus_domain || "";
  const [feed, setFeed] = useState("all");
  const [mods, setMods] = useState<Mod[]>([]);
  const [total, setTotal] = useState(0);
  const [pageSize, setPageSize] = useState(20);
  const [page, setPage] = useState(1);
  const [showRequirements, setShowRequirements] = useState(false);
  const [downloadOpen, setDownloadOpen] = useState(false);
  const [downloadFileId, setDownloadFileId] = useState<number | undefined>();
  const [details, setDetails] = useState<Details | null>(null);
  const [translation, setTranslation] = useState("");
  const [showTranslation, setShowTranslation] = useState(false);
  const [translating, setTranslating] = useState(false);
  const [translationError, setTranslationError] = useState("");
  const translationRequest = useRef(0);
  useEffect(() => { translationRequest.current++; setTranslation(""); setShowTranslation(false); setTranslating(false); setTranslationError(""); return () => { translationRequest.current++; }; }, [domain, details?.info.mod_id]);
  const translateDescription = async () => {
    if (!details || translating) return;
    if (translation) { setShowTranslation(value => !value); return; }
    const request = ++translationRequest.current;
    setTranslating(true); setTranslationError("");
    try {
      const translated = await invoke<string>("translate_nexus_description", { text: plain(details.info.description || details.info.summary) });
      if (request === translationRequest.current) { setTranslation(translated); setShowTranslation(true); }
    } catch (error) { if (request === translationRequest.current) setTranslationError(String(error)); }
    finally { if (request === translationRequest.current) setTranslating(false); }
  };
  const [filter, setFilter] = useState("");
  const [reference, setReference] = useState("");
  const [games, setGames] = useState<CatalogGame[]>([]);
  const [choosing, setChoosing] = useState(false);
  const [gameFilter, setGameFilter] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const generation = useRef(0);
  const lock = useRef(false);
  const grid = useRef<HTMLDivElement>(null);
  useEffect(() => { grid.current?.scrollTo({ top: 0 }); }, [mods]);
  const associateRef = useRef(onAssociate);
  associateRef.current = onAssociate;
  useEffect(() => { setPage(1); setTotal(0); setFilter(""); setReference(""); }, [domain, game?.id]);
  useEffect(() => {
    const request = ++generation.current;
    lock.current = false; setShowRequirements(false); setDownloadOpen(false); setDetails(null); setMods([]); setChoosing(false); setError(""); setMessage("");
    if (!domain) {
      setGames([]); setGameFilter(game?.game.name || "");
      if (!game) { setBusy(false); return () => { generation.current++; }; }
      const gameId = game.id, gameName = game.game.name || "";
      lock.current = true; setBusy(true);
      void invoke<CatalogGame[]>("list_nexus_catalog_games").then(async values => {
        if (request !== generation.current) return;
        setGames(values);
        const match = matchingCatalogGame(values, gameName);
        if (match && await associateRef.current(gameId, match.domain_name)) return;
        if (request === generation.current) setChoosing(true);
      }).catch(e => { if (request === generation.current) setError(String(e)); })
        .finally(() => { if (request === generation.current) { lock.current = false; setBusy(false); } });
      return () => { generation.current++; };
    }
    lock.current = true; setBusy(true);
    const result = feed === "all" || feed === "most_downloaded"
      ? invoke<CatalogPage>("list_nexus_catalog_page", { gameDomain: domain, feed, offset: (page - 1) * pageSize, count: pageSize })
      : invoke<Mod[]>("list_nexus_catalog_mods", { gameDomain: domain, feed }).then(mods => ({ mods, total_count: mods.length, next_offset: mods.length }));
    void result.then(page => { if (request === generation.current) { setMods(page.mods); setTotal(page.total_count);  } })
      .catch(e => { if (request === generation.current) setError(String(e)); })
      .finally(() => { if (request === generation.current) { lock.current = false; setBusy(false); } });
    return () => { generation.current++; };
  }, [domain, game?.id, game?.game.name, feed, page, pageSize]);
  const run = async (work: () => Promise<void>) => {
    if (lock.current) return;
    const request = generation.current; lock.current = true; setBusy(true); setError(""); setMessage("");
    try { await work(); } catch (e) { if (request === generation.current) setError(String(e)); }
    finally { if (request === generation.current) { lock.current = false; setBusy(false); } }
  };
  const view = (modId: number) => void run(async () => {
    const request = generation.current;
    const value = await invoke<Details>("get_nexus_catalog_mod", { gameDomain: domain, modId });
    if (request === generation.current) { setShowRequirements(false); setDownloadOpen(false); setDetails(value); }
  });

  const open = (url: string) => void run(async () => { await openUrl(url); });
  const choose = () => void run(async () => {
    const request = generation.current;
    const values = await invoke<CatalogGame[]>("list_nexus_catalog_games");
    if (request === generation.current) { setGames(values); setGameFilter(game?.game.name || ""); setChoosing(true); }
  });
  const lookup = () => {
    let id = Number(reference.trim());
    if (!/^\d+$/.test(reference.trim())) {
      try { const url = new URL(reference.trim()); const parts = url.pathname.split("/").filter(Boolean);
        if (url.protocol !== "https:" || !["www.nexusmods.com", "nexusmods.com"].includes(url.hostname) || parts[0] !== domain || parts[1] !== "mods" || !/^\d+$/.test(parts[2] || "")) throw new Error();
        id = Number(parts[2]);
      } catch { setError(text.invalidModReference); return; }
    }
    if (!Number.isSafeInteger(id) || id <= 0) { setError(text.invalidModReference); return; }
    view(id);
  };
  const pagedFeed = feed === "all" || feed === "most_downloaded";
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  const pageMods = pagedFeed ? mods : mods.slice((page - 1) * pageSize, page * pageSize);
  const visibleMods = pageMods.filter(mod => mod.name.toLocaleLowerCase().includes(filter.toLocaleLowerCase()));
  const back = () => { if (choosing) setChoosing(false); else setDetails(null); };
  const viewKey = choosing ? "association" : details ? `mod-${details.info.mod_id}` : "catalog";
  return <NexusPopup className={`nexus-catalog-popup ${viewKey === "catalog" ? "nexus-catalog-browser" : ""}`} title={choosing ? text.associateGame : details ? details.info.name : text.catalog} viewKey={viewKey} onBack={viewKey === "catalog" ? undefined : back} onClose={onClose}><section className="nexus-catalog" aria-busy={busy}>
    <div hidden={Boolean(details) || choosing}>
    <div className="nexus-panel-heading nexus-catalog-association"><h3><img className="nexus-icon" src="/nexus-mods.svg" alt="" />{text.catalog}</h3><button className="btn btn-secondary" disabled={!game || busy || Boolean(identifiedDomain)} onClick={choose}><MenuIcon name="gamepadSearch" />{text.associateGame}</button><p className="nexus-hint">{domain ? `${game?.game.name} · nexusmods.com/${domain}` : text.associationHint}</p></div>

    {error && <p className="nexus-error" role="alert">{error}</p>}
    {busy && <p role="status" className="nexus-hint">{text.loading}</p>}
    {message && <p role="status" className="nexus-hint">{message}</p>}
    {domain && <>
      <div className="nexus-tabs">{(["all", "trending", "latest_added", "latest_updated", "most_downloaded"] as const).map(value => <button className={`btn btn-secondary ${feed === value ? "active" : ""}`} key={value} disabled={busy} aria-pressed={feed === value} onClick={() => { setPage(1); setFeed(value); }}>{text[value]}</button>)}<button className="btn btn-secondary" disabled={busy} onClick={() => open(`https://www.nexusmods.com/${domain}/mods/`)}><MenuIcon name="platform" />{text.fullCatalog}</button></div>
      <><div className="nexus-fields nexus-catalog-searches"><label className="nexus-catalog-search"><MenuIcon name="search" /><input aria-label={text.filterCatalog} placeholder={text.filterCatalog} value={filter} onChange={e => setFilter(e.target.value)} /></label><form className="nexus-directory" onSubmit={e => { e.preventDefault(); lookup(); }}><input aria-label={text.modReference} placeholder={text.modReference} value={reference} onChange={e => setReference(e.target.value)} /><button className="btn btn-secondary" disabled={busy || !reference.trim()}>{text.viewMod}</button></form></div><div className="nexus-catalog-grid" ref={grid}>{visibleMods.map(mod => <article key={mod.mod_id} className="nexus-catalog-card" role="button" tabIndex={busy ? -1 : 0} aria-label={`${text.viewMod}: ${mod.name}`} aria-disabled={busy} onClick={() => { if (!busy) view(mod.mod_id); }} onKeyDown={event => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); if (!busy && !event.repeat) view(mod.mod_id); } }}><ModThumbnail key={mod.mod_id} mod={mod} /><h4>{mod.name}</h4><small>{mod.author} · {mod.version}</small>{mod.downloads != null && <small>{mod.downloads.toLocaleString()} {text.downloadCount}</small>}<p>{plain(mod.summary)}</p></article>)}</div>{!busy && !mods.length && <p className="nexus-hint">{text.noCatalogMods}</p>}<div className="nexus-catalog-pagination"><NexusPageSizeSelector value={pageSize} disabled={busy} onChange={size => { setPageSize(size); setPage(1); }} /><div><button type="button" className="btn btn-secondary" disabled={busy || page <= 1} onClick={() => setPage(value => value - 1)}>{text.previousPage}</button><span aria-live="polite">{page} / {pageCount}</span><button type="button" className="btn btn-secondary" disabled={busy || page >= pageCount} onClick={() => setPage(value => value + 1)}>{text.nextPage}</button></div></div></></>}
    </div>
    {choosing && <NexusPopup embedded title={text.associateGame} onClose={() => setChoosing(false)}><p className="nexus-hint">{text.autoAssociationHint}</p><p className="nexus-hint">{text.directory}: {game?.game.directory || "—"}</p><div className="nexus-fields"><label>{text.findNexusGame}<input value={gameFilter} onChange={e => setGameFilter(e.target.value)} /></label><div className="nexus-game-options">{filterCatalogGames(games, gameFilter).slice(0, 40).map(g => <button key={g.domain_name} className="btn btn-secondary" disabled={busy} onClick={() => void run(async () => { if (game && await onAssociate(game.id, g.domain_name)) setChoosing(false); })}>{g.name}<small>{g.domain_name}</small></button>)}</div>{!filterCatalogGames(games, gameFilter).length && <p role="status" className="nexus-hint">{text.noMatchingGame}</p>}</div>{error && <p role="alert" className="nexus-error">{error}</p>}</NexusPopup>}
    {details && <NexusPopup embedded title={details.info.name} onClose={() => { setShowRequirements(false); setDownloadOpen(false); setDetails(null); }}>
        <div>
        {error && <p role="alert" className="nexus-error">{error}</p>}{busy && <p role="status" className="nexus-hint">{text.loading}</p>}{message && <p role="status" className="nexus-hint">{message}</p>}
        <div className="nexus-mod-detail-heading"><ModThumbnail key={details.info.mod_id} mod={details.info} /><div><h3>{details.info.name}</h3><p className="nexus-hint">{details.info.author} · {details.info.version}</p><button type="button" className="btn btn-secondary nexus-translate-description" disabled={translating || !plain(details.info.description || details.info.summary).trim()} onClick={() => void translateDescription()}><MenuIcon name="language" />{translating ? text.translatingDescription : showTranslation ? text.originalDescription : text.translateDescription}</button><small className="nexus-translation-hint">{text.translationHint}</small></div></div>{translationError && <p role="alert" className="nexus-error">{translationError}</p>}<div className="nexus-mod-description" lang={showTranslation ? "pt-BR" : undefined} aria-busy={translating}>{showTranslation ? translation : plain(details.info.description || details.info.summary)}</div>
        <div className="nexus-mod-detail-actions">
          <button type="button" className="btn btn-secondary" disabled={busy} onClick={() => open(`https://www.nexusmods.com/${domain}/mods/${details.info.mod_id}`)}><MenuIcon name="platform" />{text.modPage}</button>
          <button type="button" className="btn btn-secondary" onClick={() => setShowRequirements(true)}><MenuIcon name="puzzle" />{text.requirements}{details.requirements.items.length ? ` (${details.requirements.items.length})` : ""}</button>
          <button type="button" className="btn btn-primary" disabled={busy || !game || !details.files.length} onClick={() => { setDownloadFileId(undefined); setDownloadOpen(true); }}><MenuIcon name="download" />{text.downloadNexus}</button>
        </div>
        <h4>{text.availableFiles}</h4><p className="nexus-hint">{text.browserDownloadHint}</p><div className="nexus-mod-list">{groupNexusFiles(details.files).map(({ category, files }) => { const Group = category === 4 ? "details" : "div"; return <section className="nexus-file-group" key={category}>{category !== 4 && <h4>{category === 1 ? text.mainFiles : category === 3 ? text.optionalFiles : category === 2 ? text.patchFiles : text.otherFiles}</h4>}<Group className="nexus-catalog-file-group">{category === 4 && <summary className="nexus-old-files-toggle">{text.oldFiles} ({files.length})</summary>}<div className={category === 4 ? "nexus-old-files-grid" : "nexus-file-entries"}>{files.map(file => <article className="nexus-mod-row" key={file.file_id}><div><strong>{file.name}</strong><small>{file.version} · {(file.size / 1024).toFixed(1)} MB · {file.category_name}</small><small>{file.file_name}</small>{file.description?.trim() && <p className="nexus-file-description">{plain(file.description)}</p>}</div>{category === 4 && <HoverTooltip text={`${text.downloadNexus}: ${file.version}`}><button type="button" className="btn btn-primary nexus-download-icon" aria-label={`${text.downloadNexus}: ${file.name} · ${file.version}`} disabled={busy || !game} onClick={() => { setDownloadFileId(file.file_id); setDownloadOpen(true); }}><MenuIcon name="download" /></button></HoverTooltip>}</article>)}</div></Group></section>; })}</div>
        {!details.files.length && <p className="nexus-hint">{text.noCatalogFiles}</p>}
        <button className="btn btn-secondary" disabled={busy || !game || Boolean(game.running)} onClick={() => void run(async () => { const request = generation.current; const source = await openFilePicker(text.importDownloaded, nexusArchiveFormats); if (source && game && await onImport(game.id, source) && request === generation.current) setMessage(text.archiveImported); })}><MenuIcon name="plusCircle" />{text.importDownloaded}</button>
        </div>
        {showRequirements && <NexusPopup className="nexus-requirements-popup" title={text.requirements} onClose={() => setShowRequirements(false)}><h3>{details.info.name}</h3><NexusRequirements game={game} requirements={details.requirements} onOpen={open} disabled={busy} /></NexusPopup>}
        {downloadOpen && game && <NexusDownloadQueue game={game} domain={domain} modId={details.info.mod_id} details={details} fileId={downloadFileId} onClose={() => setDownloadOpen(false)} onStarted={() => { setDownloadOpen(false); setMessage(text.queueStarted); }} />}
      </NexusPopup>}
  </section></NexusPopup>;
}
