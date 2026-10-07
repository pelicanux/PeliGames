import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import type { ProtonFamily } from "../services/protonService";

export function ProtonPreference({ family, onChange, onUpdate }: { family: ProtonFamily; onChange: (family: ProtonFamily) => void; onUpdate: () => void }) {
  const { t } = useI18n();
  return <section className="proton-preference" aria-labelledby="proton-preference-heading">
    <div className="proton-preference-heading">
      <h3 id="proton-preference-heading">{t("runners", "title")}</h3>
      <button type="button" className="proton-preference-update" onClick={onUpdate}><MenuIcon name="download" />{t("runners", "updateProton")}</button>
    </div>
    <div className="backend-options proton-preference-options" role="group" aria-label={t("runners", "family")}>
      {(["ge-proton", "cachyos-proton"] as const).map(option => <button type="button" key={option}
        className={`backend-option proton-preference-option ${family === option ? "active" : ""}`} aria-pressed={family === option} onClick={() => onChange(option)}>
        <h4>{option === "ge-proton" ? "GE-Proton" : "Proton CachyOS"}</h4>
        <span>{option === "ge-proton" ? "GloriousEggroll" : "CachyOS"}</span>
      </button>)}
    </div>
  </section>;
}
