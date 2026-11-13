import {MenuIcon,PeliRunProfile,useI18n} from "./environment";
import type {GameReference} from "./environment";

/** Uses the same launcher-neutral profile exporter as the game's Options page. */
export function OptiScalerUsage({game,installed,blocked}:{game:GameReference;installed:boolean;blocked:boolean}) {
 const {t}=useI18n();
 const steps=[
  {title:"usage_install",text:"usage_install_hint",icon:"download"},
  {title:"usage_configure",text:"usage_configure_hint",icon:"sliders"},
  {title:"usage_launch",text:"usage_launch_hint",icon:"play"},
 ] as const;
 return <div className="upscaler-usage">
  {steps.map(step=><section className="upscaler-usage-step" key={step.title}>
   <MenuIcon name={step.icon}/><div><h4>{t("upscaler",step.title)}</h4><p>{t("upscaler",step.text)}</p></div>
  </section>)}
  <section className="upscaler-usage-launcher">
   <h4>{t("upscaler","usage_other_launcher")}</h4>
   <p>{t("upscaler","usage_other_launcher_hint")}</p>
   {!installed&&<p className="upscaler-usage-note">{t("upscaler","usage_install_first")}</p>}
   <PeliRunProfile game={game} disabled={blocked||!installed} inline actionLabel={t("upscaler","usage_prepare_command")}/>
  </section>
 </div>;
}
