import { useState, type ReactNode } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { MenuIcon } from "../../components/MenuIcon";
import { ReportModal } from "./ReportModal";
import type { FeatureReportContext } from "./reportContext";
import "./reports.css";
export function FeatureReportSection({context, label, children}: {context: FeatureReportContext; label: string; children: ReactNode}) {
  const {t: tx} = useI18n();

  // Freeze the selected installation and area when opening; a background tab
  // change must never replace the report currently being reviewed.
  const [target, setTarget] = useState<FeatureReportContext>();
  return <section className="feature-report-section">
    <div className="feature-report-toolbar"><button type="button" className="modern-mini-btn feature-report-button" onClick={()=>setTarget(context)} aria-label={(tx("ui-feature-report-section", "report_a_bug")) + " · " + label}><MenuIcon name="bug" />{tx("ui-feature-report-section", "report_a_bug")}</button></div>
    {children}
    {target && <ReportModal target={{kind:"feature", context:target, label}} onClose={()=>setTarget(undefined)} />}
  </section>;
}
