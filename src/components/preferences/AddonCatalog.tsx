import { useI18n } from "../../i18n/I18nContext";
import { experimentalFrameGeneration } from "../../services/releaseFeatures";
import { MenuIcon } from "../MenuIcon";
import {addons, addonAvailable, addonLabel, type AddonId} from "./addonCatalogData";
import "./addonCatalog.css";

export function AddonCatalog({ onOpen, disabled }: { onOpen: (value: AddonId) => void; disabled: boolean }) {
  const {t: tx, language} = useI18n();

  return <nav className="addon-catalog mod-workspace" aria-label={tx("ui-addon-catalog", "choose_an_add_on")}>
    {addons.filter(addon => addonAvailable(addon.id)).map(addon => {
      const unavailable = addon.id === "mako" && !experimentalFrameGeneration;
      return <button type="button" className="addon-catalog-button" key={addon.id} id={`addon-button-${addon.id}`}
        aria-label={addonLabel(addon.id, language)} disabled={disabled || unavailable} onClick={() => onOpen(addon.id)}>
        <span className="mod-component-icon"><MenuIcon name={addon.icon}/></span>
        <span className="addon-catalog-copy"><strong>{addonLabel(addon.id, language)}</strong><small>{unavailable ? (tx("ui-addon-catalog", "in_development")) : tx("addonCatalogData", addon.labelKey)}</small></span>
        <span className="addon-catalog-arrow" aria-hidden="true">›</span>
      </button>;
    })}
  </nav>;
}
