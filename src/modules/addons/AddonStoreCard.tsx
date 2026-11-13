import {HoverTooltip} from "../../components/HoverTooltip";
import {ProjectLink} from "../../components/ProjectLink";
import {AddonProjectIcon,projectReference} from "./projectIdentity";
import {useState} from "react";
import {MenuIcon} from "../../components/MenuIcon";
import {SelectionMenu} from "../../components/SelectionMenu";
import {ModOptionSwitch} from "../../components/ModOptionRow";
import {useI18n} from "../../i18n/I18nContext";
import {contributionRoute, type AddonOverview, type AddonRoute} from "./contracts";
import {manifestCategory} from "./navigation";
import type {AddonStoreController, StoreSnapshot} from "./store";

type Entry = AddonOverview["catalog"]["addons"][number];
interface Props {
  entry: Entry;
  overview: AddonOverview;
  snapshot: StoreSnapshot;
  controller: AddonStoreController;
  blocked: boolean;
  installedView?: boolean;
  onOpen: (route: AddonRoute) => void;
}
export function AddonStoreCard({entry: {manifest, artifacts}, overview, snapshot, controller, blocked, onOpen, installedView=false}: Props) {
  const {t,language} = useI18n();
  const [removing, setRemoving] = useState(false);
  const installed = overview.state.addons[manifest.id];
  const selected = installed?.versions[installed.selected ?? ""];
  const contribution = overview.contributions.find(item => item.id === manifest.id && item.contribution.target === "settings");
  const available = Boolean(artifacts[overview.platform]);
  const update = Boolean(selected && manifest.version !== installed?.selected);
  const category = manifestCategory(manifest);
  const versions=Object.keys(installed?.versions??{});
  const progress = snapshot.progress?.id === manifest.id ? snapshot.progress : undefined;
  const installing = snapshot.busy===manifest.id && (!selected || update || Boolean(progress));
  const downloadLabel = installing ? progress&&progress.phase!=="downloading"?"installing":"downloading" : !available ? "unavailable" : update ? "update" : "install";

  return <article className="addon-store-card" data-category={category} data-enabled={Boolean(selected && installed?.enabled)}>
    <header>
      <AddonProjectIcon manifest={manifest}/>
      <div className="addon-store-card-identity"><h3>{manifest.name}</h3>
        <div className="addon-store-project"><ProjectLink project={projectReference(manifest)}>{t("addons","project_link")}</ProjectLink></div>
        <small className="addon-store-card-category"><span>{t("addons", category)}</span>v{selected ? installed?.selected : manifest.version}{selected?.local && <> · {t("addons","local_package")}</>}</small>
      </div>
      {selected && installedView && <ModOptionSwitch appearance="rail" label={manifest.name} enabled={installed!.enabled}
        disabled={blocked} onToggle={() => void controller.enable(manifest.id, !installed!.enabled)}/>}
    </header>
    <p>{manifest.contributions.map(item=>item.descriptions?.[language]||item.descriptions?.[language.split("-")[0]]).find(Boolean)||manifest.summary}</p>
    {selected && versions.length>1 && <div className="addon-store-version">
      <SelectionMenu appearance="compact" label={t("addons", "version")} value={installed!.selected!}
        options={versions.map(value => ({value, label: value}))}
        disabled={blocked} onChange={version => void controller.select(manifest.id, version)}/>
    </div>}

    <footer data-confirming={removing || undefined}>{removing ? <>
      <p className="addon-store-remove-message" title={t("addons","remove_confirm")}>{t("addons", "remove_confirm")}</p>
      <div className="addon-store-confirm-actions">
      <HoverTooltip text={t("addons","cancel")} tooltipClassName="modern-tools-tooltip"><button type="button" className="mod-action-quiet addon-store-icon-action" aria-label={t("addons","cancel")} disabled={blocked} onClick={() => setRemoving(false)}><MenuIcon name="close"/></button></HoverTooltip>
      <HoverTooltip text={t("addons","confirm")} tooltipClassName="modern-tools-tooltip"><button type="button" className="mod-action-quiet danger addon-store-icon-action" aria-label={t("addons","confirm")} disabled={blocked} onClick={() => {
        setRemoving(false);
        void controller.remove(manifest.id);
      }}><MenuIcon name="check"/></button></HoverTooltip></div>
    </> : <>
      {selected&&<span className="addon-store-status" data-enabled={installed?.enabled}>{t("addons",installed?.enabled?"enabled":"disabled")}</span>}
      <div className="addon-store-card-actions">
      {contribution && <HoverTooltip text={t("addons","configure")} tooltipClassName="modern-tools-tooltip"><button type="button" className="mod-action-quiet addon-store-icon-action addon-store-round-action" aria-label={t("addons","configure")} disabled={blocked}
        onClick={() => onOpen(contributionRoute(contribution))}><MenuIcon name="settings"/></button></HoverTooltip>}
      {(!selected || update || installing) && <HoverTooltip text={t("addons",downloadLabel)} tooltipClassName="modern-tools-tooltip"><button type="button" className="mod-action-quiet addon-store-install" disabled={blocked || !available} aria-label={t("addons",downloadLabel)} aria-busy={installing} data-working={installing||undefined}
        onClick={() => void controller.install(manifest.id)}>
        {installing&&<span className="addon-install-indicator"><svg className="addon-install-ring" viewBox="0 0 40 40" aria-hidden="true"><circle className="addon-install-ring-track" cx="20" cy="20" r="18"/><circle className="addon-install-ring-value" cx="20" cy="20" r="18" pathLength="100" strokeDasharray={progress?.total?`${Math.min(100,Math.round(progress.downloaded/progress.total*100))} 100`:undefined}/></svg>
        <MenuIcon name={progress&&progress.phase!=="downloading"?"settings":"download"}/></span>}
        <span>{t("addons",downloadLabel)}</span>
      </button></HoverTooltip>}
      {selected && <HoverTooltip text={t("addons","uninstall")} tooltipClassName="modern-tools-tooltip"><button type="button" className="mod-action-quiet danger addon-store-icon-action" aria-label={t("addons","uninstall")} disabled={blocked} onClick={() => setRemoving(true)}><MenuIcon name="trash"/></button></HoverTooltip>}
      </div>
    </>}</footer>
  </article>;
}
