import type { ReactNode } from "react";
import { MenuIcon } from "./MenuIcon";
import { HoverTooltip } from "./HoverTooltip";

type Icon = Parameters<typeof MenuIcon>[0]["name"];

/** One line of the mod panel: icon, name and info balloon on the left, the control on the right. */
export function ModOptionRow({ icon, label, hint, dimmed = false, children }: { icon: Icon; label: string; hint: string; dimmed?: boolean; children: ReactNode }) {
  return <div className={`mod-option-row${dimmed ? " dimmed" : ""}`}>
    <span className="mod-option-label"><MenuIcon name={icon} /><span>{label}</span>
      <HoverTooltip text={hint}><button type="button" className="installation-type-info" aria-label={hint}><MenuIcon name="info" /></button></HoverTooltip>
    </span>
    <div className="mod-option-control">{children}</div>
  </div>;
}

/** The panel's on/off switch, compact for a row. The switch shows its state;
 * screen readers get it from role="switch" and aria-checked. */
export function ModOptionSwitch({ label, enabled, disabled, onToggle }: { label: string; enabled: boolean; disabled?: boolean; onToggle: () => void }) {
  return <button type="button" role="switch" aria-checked={enabled} aria-label={label}
    className={`neural-startup-switch mod-option-switch ${enabled ? "enabled" : ""}`} disabled={disabled} onClick={onToggle}>
    <span className="neural-switch-track" aria-hidden="true"><span /></span>
  </button>;
}
