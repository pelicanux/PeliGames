import {useState} from "react";
import {MenuIcon} from "../../components/MenuIcon";
import {useI18n} from "../../i18n/I18nContext";
import {addonStore,useAddonStore} from "./store";
import "./addonGuide.css";

/** Small interface excerpts, translated with the host rather than baked into images. */
export function AddonGuideVisual() {
  const {t}=useI18n();
  return <div className="addon-guide-visual" aria-label={t("addons","guide_visual_label")}>
    <figure>
      <div className="addon-guide-excerpt">
        <div className="addon-guide-demo-heading"><MenuIcon name="addonStore"/><span>{t("addons","store")}</span><small>{t("addons","explore")}</small></div>
        <div className="addon-guide-demo-identity"><span className="addon-guide-demo-mark"><MenuIcon name="graphics"/></span><div><strong>OptiScaler</strong><small>{t("addons","mods")}</small></div></div>
        <div className="addon-guide-demo-footer"><span className="addon-guide-demo-lines"/><span className="addon-guide-demo-action" title={t("addons","install")}><MenuIcon name="download"/></span></div>
      </div>
      <figcaption>{t("addons","guide_choose")}</figcaption>
    </figure>
    <figure>
      <div className="addon-guide-excerpt">
        <div className="addon-guide-demo-heading"><MenuIcon name="checkCircle"/><span>{t("addons","installed_filter")}</span></div>
        <div className="addon-guide-demo-identity"><span className="addon-guide-demo-mark"><MenuIcon name="graphics"/></span><div><strong>OptiScaler</strong><small className="addon-guide-demo-enabled">{t("addons","enabled")}</small></div><span className="addon-guide-demo-switch"/></div>
        <div className="addon-guide-demo-footer"><span>{t("addons","configure")}</span><MenuIcon name="settings"/></div>
      </div>
      <figcaption>{t("addons","guide_install")}</figcaption>
    </figure>
    <figure>
      <div className="addon-guide-excerpt">
        <div className="addon-guide-demo-heading"><MenuIcon name="gamepad"/><span>{t("addons","guide_game_details")}</span></div>
        <div className="addon-guide-demo-tabs"><span data-active>{t("addons","mods")}</span><span>{t("addons","frame-generation")}</span><span>{t("addons","captures")}</span></div>
        <div className="addon-guide-demo-footer addon-guide-demo-destination"><MenuIcon name="graphics"/><strong>OptiScaler</strong><MenuIcon name="chevronDown"/></div>
      </div>
      <figcaption>{t("addons","guide_use")}</figcaption>
    </figure>
  </div>;
}

export function AddonGuide({onOpenHub,onSkip}: {onOpenHub:()=>void;onSkip:()=>void}) {
  const {t}=useI18n();
  const snapshot=useAddonStore();
  const [pending,setPending]=useState(false);
  const finish=async(open:boolean)=>{
    if(pending||snapshot.busy)return;
    setPending(true);
    try {
      if(!await addonStore.complete())return;
      if(open)onOpenHub();else onSkip();
    } finally {setPending(false);}
  };
  const busy=pending||Boolean(snapshot.busy);
  return <section className="addon-guide">
    <span className="addon-guide-eyebrow"><MenuIcon name="puzzle"/>{t("addons","guide_eyebrow")}</span>
    <h2>{t("addons","guide_title")}</h2>
    <p className="addon-guide-intro">{t("addons","guide_description")}</p>
    <AddonGuideVisual/>
    <div className="addon-guide-explanation"><p>{t("addons","guide_tabs")}</p><p>{t("addons","guide_optional")}</p></div>
    <p className="addon-guide-note"><MenuIcon name="info"/>{t("addons","guide_components")}</p>
    {snapshot.error&&<p role="alert" className="preferences-error">{snapshot.error}</p>}
    <div className="addon-guide-actions"><button type="button" disabled={busy} onClick={()=>void finish(false)}>{t("addons","guide_later")}</button><button type="button" className="addon-guide-primary" disabled={busy} onClick={()=>void finish(true)}><MenuIcon name="addonStore"/>{t("addons","guide_open")}</button></div>
  </section>;
}
