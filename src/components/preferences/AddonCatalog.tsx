import { useI18n } from "../../i18n/I18nContext";
import {ComponentCatalog} from "../ComponentCatalog";
import {addons, addonAvailable, addonLabel, type AddonId} from "./addonCatalogData";
import {InstalledAddonCatalog} from "../../modules/addons/InstalledAddonCatalog";
import type {AddonRoute} from "../../modules/addons/contracts";
import "./addonCatalog.css";

export function AddonCatalog({ onOpen, disabled, onOpenExternal }: { onOpen: (value: AddonId) => void; disabled: boolean; onOpenExternal?: (route: AddonRoute) => void }) {
  const {t: tx, language} = useI18n();

  return <><ComponentCatalog label={tx("ui-addon-catalog", "choose_an_add_on")} disabled={disabled} onOpen={onOpen}
    cards={addons.filter(addon => addonAvailable(addon.id)).map(addon => {

      return {id: addon.id, label: addonLabel(addon.id, language), description: tx("addonCatalogData", addon.labelKey),
        icon: addon.icon, buttonId: `addon-button-${addon.id}`};
    })} />{onOpenExternal && <InstalledAddonCatalog target="settings" disabled={disabled} onOpen={onOpenExternal}/>}</>;
}
