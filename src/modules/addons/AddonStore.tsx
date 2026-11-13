import {ProjectLink} from "../../components/ProjectLink";
import {PELIGAMES_ISSUES_URL} from "../../services/communityLinks";
import {SelectionMenu} from "../../components/SelectionMenu";
import {HubBanners} from "./HubBanners";
import {useEffect, useState} from "react";
import {MenuIcon} from "../../components/MenuIcon";
import {useI18n} from "../../i18n/I18nContext";
import type {AddonCategory, AddonRoute} from "./contracts";
import {categories, manifestCategory, storeEntries} from "./navigation";
import {addonStore, useAddonStore, type AddonStoreController} from "./store";
import {AddonLocalImport} from "./AddonLocalImport";
import {AddonStoreCard} from "./AddonStoreCard";
import "./store.css";

interface Props {
  disabled?: boolean;
  onBusyChange?: (busy: boolean) => void;
  onOpen: (route: AddonRoute) => void;
  controller?: AddonStoreController;
}
export function AddonStore({disabled = false, onBusyChange, onOpen, controller = addonStore}: Props) {
  const {t} = useI18n();
  const snapshot = useAddonStore(controller);
  const {overview, busy, error} = snapshot;
  const [category, setCategory] = useState<AddonCategory | "all">("all");
  const [search, setSearch] = useState("");
  const [installedOnly,setInstalledOnly]=useState(false);
  const [showImport,setShowImport]=useState(false);
  useEffect(() => {void controller.refreshCatalog()}, [controller]);
  useEffect(() => {
    onBusyChange?.(Boolean(busy));
    return () => onBusyChange?.(false);
  }, [busy, onBusyChange]);
  const available = overview ? storeEntries(overview) : [];
  const entries = available.filter(entry =>
    (category === "all" || manifestCategory(entry.manifest) === category) &&
    (!installedOnly || Boolean(overview?.state.addons[entry.manifest.id]?.selected)) &&
    `${entry.manifest.name} ${entry.manifest.summary}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()),
  );
  const blocked = disabled || Boolean(busy);

  const installedCount=available.filter(entry=>Boolean(overview?.state.addons[entry.manifest.id]?.selected)).length;
  return <section className="addon-store addon-hub mod-workspace" aria-label={t("addons", "store")}>
    <div className="addon-hub-content">
    <HubBanners disabled={blocked}/>
    <div className="addon-hub-navigation">
      <nav className="addon-hub-tabs" aria-label={t("addons","browse")}>
        <button type="button" aria-pressed={!installedOnly} onClick={()=>setInstalledOnly(false)}><MenuIcon name="grid"/><span>{t("addons","explore")}</span><small>{available.length}</small></button>
        <button type="button" aria-pressed={installedOnly} onClick={()=>setInstalledOnly(true)}><MenuIcon name="checkCircle"/><span>{t("addons","installed_filter")}</span><small>{installedCount}</small></button>
      </nav>
      <div className="addon-hub-filters">
        <label className="addon-store-search"><MenuIcon name="search"/>
          <input type="search" value={search} onChange={event => setSearch(event.target.value)} placeholder={t("addons", "search")} aria-label={t("addons", "search")}/>
        </label>
      <SelectionMenu appearance="compact" className="addon-hub-category" minMenuWidth={190} menuAlign="right" leadingIcon={<MenuIcon name="filter"/>} label={t("addons","categories")} value={category}
        options={[{value:"all",label:t("addons","all")},...categories.map(id=>({value:id,label:t("addons",id)}))]}
        onChange={value=>setCategory(value as AddonCategory | "all")}/>
        <button type="button" className="mod-action-quiet addon-hub-import" title={t("addons","import_zip")} aria-label={t("addons","import_zip")} aria-expanded={showImport} disabled={blocked} onClick={()=>setShowImport(!showImport)}><MenuIcon name="packageImport"/></button>
        <button type="button" className="mod-action-quiet addon-hub-refresh" disabled={blocked} title={t("addons","refresh")} aria-label={t("addons","refresh")} onClick={()=>void controller.refreshCatalog()}><MenuIcon name="repair"/></button>
      </div>
    </div>
    {showImport&&<AddonLocalImport controller={controller} disabled={blocked}/>}
    {snapshot.loading && !overview && <p role="status">{t("addons", "loading")}</p>}
    {overview?.catalog_warning && <p role="alert" className="preferences-error">{t("addons", "catalog_cache_invalid")}</p>}
    {error && <p role="alert" className="preferences-error">{error}</p>}
    {!snapshot.loading && !entries.length && <div className="addon-store-empty"><MenuIcon name={search ? "search" : "addonStore"}/><p>{t("addons", search ? "no_results" : installedOnly ? "no_installed" : "no_packages")}</p></div>}
    <p className="addon-hub-independence">{t("addons","independence_notice")} <ProjectLink project={{name:"PeliGames · GitHub",url:PELIGAMES_ISSUES_URL}}>{t("addons","report_github")}</ProjectLink></p>
    <div className="addon-store-grid">{overview && entries.map(entry =>
      <AddonStoreCard key={entry.manifest.id} entry={entry} overview={overview} snapshot={snapshot} controller={controller} blocked={blocked} onOpen={onOpen} installedView={installedOnly}/>,
    )}</div>
    </div>
  </section>;
}
