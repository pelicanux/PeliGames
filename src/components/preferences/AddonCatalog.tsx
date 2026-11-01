import { useI18n } from "../../i18n/I18nContext";
import { experimentalFrameGeneration } from "../../services/releaseFeatures";
import { MenuIcon } from "../MenuIcon";
import {addons, addonAvailable, addonLabel, type AddonId} from "./addonCatalogData";
import "./addonCatalog.css";


export function AddonCatalog({ onOpen, disabled }: { onOpen: (value: AddonId) => void; disabled: boolean }) {
  const pt = useI18n().language === "pt";
  return <nav className="addon-catalog mod-workspace" aria-label={pt ? "Escolher complemento" : "Choose an add-on"}>
    {addons.filter(addon => addonAvailable(addon.id)).map(addon => {
      const unavailable = addon.id === "mako" && !experimentalFrameGeneration;
      return <button type="button" className="addon-catalog-button" key={addon.id} id={`addon-button-${addon.id}`}
        aria-label={addonLabel(addon.id, pt)} disabled={disabled || unavailable} onClick={() => onOpen(addon.id)}>
        <span className="mod-component-icon"><MenuIcon name={addon.icon}/></span>
        <span className="addon-catalog-copy"><strong>{addonLabel(addon.id, pt)}</strong><small>{unavailable ? (pt ? "Em desenvolvimento" : "In development") : pt ? addon.pt : addon.en}</small></span>
        <span className="addon-catalog-arrow" aria-hidden="true">›</span>
      </button>;
    })}
  </nav>;
}
