import { useState, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../../components/MenuIcon";
import { ReportModal } from "./ReportModal";
import type { FeatureReportContext } from "./reportContext";
import "./reports.css";
export type FeatureReportContent=ReactNode|((action:ReactNode)=>ReactNode);
export function FeatureReportSection({context, label, children}: {context: FeatureReportContext; label: string; children: FeatureReportContent}) {
  const {t: tx} = useI18n();

  // Freeze the selected installation and area when opening; a background tab
  // change must never replace the report currently being reviewed.
  const [target, setTarget] = useState<FeatureReportContext>();
  const action=<button type="button" className="modern-mini-btn feature-report-button" onClick={()=>setTarget(context)} aria-label={(tx("ui-feature-report-section", "report_a_bug")) + " · " + label}><MenuIcon name="bug" />{tx("ui-feature-report-section", "report_a_bug")}</button>;
  return <section className="feature-report-section">
    {typeof children==="function"?children(action):<><div className="feature-report-toolbar">{action}</div>{children}</>}
    {target && <ReportModal target={{kind:"feature", context:target, label}} onClose={()=>setTarget(undefined)} />}
  </section>;
}
