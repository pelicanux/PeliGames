import type { ReactNode } from "react";
import { MenuIcon } from "./MenuIcon";
import { HoverTooltip } from "./HoverTooltip";
import "./modSwitch.css";

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

/** Compact on/off control with an accessible name and checked state. */
export function ModOptionSwitch({ label, enabled, disabled, onToggle, appearance }: { label: string; enabled: boolean; disabled?: boolean; onToggle: () => void; appearance?: "rail" }) {
  return <button type="button" role="switch" aria-checked={enabled} aria-label={label}
    className={`neural-startup-switch mod-option-switch ${enabled ? "enabled" : ""}${appearance === "rail" ? " mod-option-switch--rail" : ""}`} disabled={disabled} onClick={onToggle}>
    <span className="neural-switch-track" aria-hidden="true"><span /></span>
  </button>;
}
