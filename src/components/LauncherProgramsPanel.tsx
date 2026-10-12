import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { launcherShortcuts, addShortcutToLibrary, removeShortcutFromLibrary, profileOptions, setProfileKind, type LauncherShortcut, type ProfileOptions, type ProfileKind } from "../services/gameExecution";
import { ConfirmModal } from "./ConfirmModal";
import { notifyError } from "../services/notifications";
import { CoverContextMenuPanel } from "./CoverContextMenu";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";

/** Icon of a program, read from its executable. */
function ProgramIcon({ source }: { source?: string | null }) {
  const [url, setUrl] = useState("");
  useEffect(() => {
    if (!source) return;
    let active = true, created = "";
    void invoke<number[] | null>("get_pelinstall_icon", { path: source }).then(bytes => {
      if (!active || !bytes?.length) return;
      created = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" }));
      setUrl(created);
    }).catch(() => undefined);
    return () => { active = false; if (created) URL.revokeObjectURL(created); };
  }, [source]);
  return url ? <img className="launcher-program-icon" src={url} alt="" /> : <span className="launcher-program-icon launcher-program-icon-fallback"><MenuIcon name="gamepad" /></span>;
}

/** Games launchers (EA app, Battle.net, Ubisoft Connect…) installed in this prefix, opened straight from here. */
export function LauncherProgramsPanel({ path, disabled, onStart }: { path: string; disabled: boolean; onStart: (id: string) => Promise<void> }) {
  const { t } = useI18n();
  const [items, setItems] = useState<LauncherShortcut[] | null>(null);
  const [error, setShownError] = useState("");
  const setError = (message: string) => { setShownError(message); if (message) notifyError(t("installedSettings", "programsTitle"), message); };
  const [status, setStatus] = useState("");
  const [menu, setMenu] = useState<{ item: LauncherShortcut; x: number; y: number } | null>(null);
  const [revision, setRevision] = useState(0);
  const [options, setOptions] = useState<ProfileOptions | null>(null);
  const [working, setWorking] = useState(false);
  const [removing, setRemoving] = useState<LauncherShortcut | null>(null);
  useEffect(() => { void profileOptions(path).then(setOptions).catch(() => setOptions(null)); }, [path]);
  const chooseKind = (kind: ProfileKind) => {
    if (!options || options.kind === kind || working) return;
    void setProfileKind(path, kind).then(setOptions).catch(e => setError(String(e)));
  };
  useEffect(() => {
    let active = true;
    if (!revision) setItems(null);
    setError("");
    void launcherShortcuts(path).then(list => { if (active) setItems(list); }).catch(e => { if (active) { setItems([]); setError(String(e)); } });
    return () => { active = false; };
  }, [path, revision]);
  // Right click: the game joins the library (its own cover and Start button) or leaves it.
  // Adding creates the game's own profile (its prefix, mirrored from the launcher's).
  const toggleLibrary = async (item: LauncherShortcut) => {
    setMenu(null); setRemoving(null); setError("");
    if (working) return;
    setWorking(true);
    setStatus(item.in_library ? "" : `${t("installedSettings", "profileCreating")} ${item.name}…`);
    try {
      if (item.in_library) await removeShortcutFromLibrary(path, item.id); else await addShortcutToLibrary(path, item.id);
      setStatus(`${item.name}: ${t("installedSettings", item.in_library ? "programsRemoved" : "programsAdded")}`);
      setRevision(value => value + 1);
      window.dispatchEvent(new Event("peligamesLibraryChanged"));
    } catch (e) { setError(String(e)); setStatus(""); }
    finally { setWorking(false); }
  };
  const start = async (item: LauncherShortcut) => {
    setError(""); setStatus(`${t("installedSettings", "programsOpening")} ${item.name}…`);
    try { await onStart(item.id); }
    catch (e) { setError(String(e)); setStatus(""); }
  };
  return <section className="launcher-programs game-info-panel menu-glass-panel">
    <div style={{ display: "flex", alignItems: "center", gap: ".5rem" }}><MenuIcon name="gamepad" /><span style={{ fontWeight: "bold" }}>{t("installedSettings", "programsTitle")}</span></div>
    <p className="launcher-programs-hint">{t("installedSettings", "programsHint")}</p>
    {options && <div className="launcher-profile-kind" role="radiogroup" aria-label={t("installedSettings", "profileKind")}>
      <span>{t("installedSettings", "profileKind")}</span>
      <button type="button" role="radio" aria-checked={options.kind === "overlay"} className={options.kind === "overlay" ? "active" : ""} disabled={!options.overlay || working}
        title={t("installedSettings", "profileOverlayHint")} onClick={() => chooseKind("overlay")}>Overlay</button>
      <button type="button" role="radio" aria-checked={options.kind === "reflink"} className={options.kind === "reflink" ? "active" : ""} disabled={!options.reflink || working}
        title={options.reflink ? t("installedSettings", "profileReflinkHint") : t("installedSettings", "profileReflinkUnavailable")} onClick={() => chooseKind("reflink")}>Reflink</button>
    </div>}
    {items === null && <p role="status">{t("installedSettings", "programsLoading")}</p>}
    {items?.length === 0 && !error && <p className="launcher-programs-hint">{t("installedSettings", "programsEmpty")}</p>}
    {!!items?.length && <div className="launcher-programs-grid">
      {items.map(item => <button key={item.id} type="button" className="launcher-program" disabled={disabled} title={item.name} onClick={() => void start(item)}
        onContextMenu={event => { event.preventDefault(); setMenu({ item, x: event.clientX, y: event.clientY }); }}>
        <ProgramIcon source={item.icon} />
        <span className="launcher-program-name">{item.name}</span>
        {item.in_library && <span className="launcher-program-badge" title={t("installedSettings", "programsInLibrary")}><MenuIcon name="check" /></span>}
      </button>)}
    </div>}
    {menu && <CoverContextMenuPanel x={menu.x} y={menu.y} onClose={() => setMenu(null)}>
      <button disabled={disabled} onClick={() => { const item = menu.item; setMenu(null); void start(item); }}><MenuIcon name="play" />{t("installedSettings", "programsOpen")}</button>
      {menu.item.in_library
        ? <button className="cover-action-danger" disabled={working} onClick={() => { setRemoving(menu.item); setMenu(null); }}><MenuIcon name="trash" />{t("installedSettings", "programsRemove")}</button>
        : <button disabled={working} onClick={() => void toggleLibrary(menu.item)}><MenuIcon name="plusCircle" />{t("installedSettings", "programsAdd")}</button>}
    </CoverContextMenuPanel>}
    {removing && <ConfirmModal title={t("installedSettings", "programsRemove")} isDanger busy={working}
      message={`${removing.name}\n${t("installedSettings", "profileRemoveWarning")}`} confirmText={t("installedSettings", "programsRemove")} cancelText={t("confirm", "cancel")}
      onCancel={() => setRemoving(null)} onConfirm={() => void toggleLibrary(removing)} />}
    {status && <p role="status" className="launcher-programs-hint">{status}</p>}
    {error && <p role="alert" className="preferences-error">{error}</p>}
  </section>;
}
