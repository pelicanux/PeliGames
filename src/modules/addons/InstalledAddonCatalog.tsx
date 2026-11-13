import {useEffect, useState} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {ComponentCatalog} from "../../components/ComponentCatalog";
import {addonBridge, readContributions} from "./bridge";
import type {AddonRoute, AddonTarget, UiContribution} from "./contracts";

export function InstalledAddonCatalog({target, disabled, onOpen}: {target: AddonTarget; disabled: boolean; onOpen: (route: AddonRoute) => void}) {
  const {t} = useI18n();
  const [entries, setEntries] = useState<UiContribution[]>([]);
  const [error, setError] = useState(false);
  useEffect(() => {
    let closed = false, revision = 0;
    let unsubscribe: (() => void) | undefined;
    const refresh = () => {
      const request = ++revision;
      void readContributions().then(items => {if (!closed && request === revision) {setEntries(items.filter(item => item.contribution.target === target)); setError(false)}}, () => {if (!closed && request === revision) setError(true)});
    };
    void addonBridge.observe(refresh).then(release => {if (closed) release(); else {unsubscribe = release; refresh()}}, refresh);
    return () => {closed = true; unsubscribe?.()};
  }, [target]);
  if (error) return <p className="preferences-error" role="alert">{t("addons", "catalog_failed")}</p>;
  if (!entries.length) return null;
  return <section className="addon-installed-catalog"><h3>{t("addons", "installed")}</h3><ComponentCatalog label={t("addons", "installed")} disabled={disabled}
    cards={entries.map((item, index) => ({id: String(index), label: item.contribution.title, description: `${item.name} · ${item.version}`, icon: "puzzle", buttonId: `external-addon-${item.id}-${item.contribution.route}`}))}
    onOpen={index => {const item = entries[Number(index)]; if (item) onOpen({id: item.id, ...item.contribution, project: {name: item.name, url: item.repository}})}}/></section>;
}
