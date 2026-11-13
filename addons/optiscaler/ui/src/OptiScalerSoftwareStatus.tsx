import {useI18n} from "./environment";
import type {OptiScalerSoftware} from "./settings";
import "./softwareStatus.css";

const names={optiscaler:"OptiScaler",dlss:"DLSS",fsr:"FSR",xess:"XeSS",fakenvapi:"FakeNVAPI",streamline:"Streamline SDK",optipatcher:"OptiPatcher"} as const;
export function OptiScalerSoftwareStatus({items,scope}:{items:OptiScalerSoftware[];scope:"package"|"game"}){
 const {t}=useI18n();
 return <section className="optiscaler-software-status" aria-label={t("upscaler",scope==="game"?"software_game":"software_package")}>
  <h4>{t("upscaler",scope==="game"?"software_game":"software_package")}</h4>
  <dl>{items.map(item=><div key={item.id} data-present={item.present||item.integrated}>
   <dt><span className="optiscaler-software-dot" aria-hidden="true"/>{names[item.id]}</dt>
   <dd>{item.integrated?t("upscaler","software_integrated"):!item.present?t("upscaler","software_absent"):item.version||t("upscaler","software_present")}</dd>
  </div>)}</dl>
  <p>{t("upscaler","software_hint")}</p>
 </section>;
}
