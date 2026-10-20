import type { CSSProperties, ComponentProps } from "react";
import { MenuIcon } from "./MenuIcon";
import "./InstallationActionIcon.css";

/** The same installation action artwork in the launcher and standalone assistant. */
export function InstallationActionIcon({ name, from, to }: { name: ComponentProps<typeof MenuIcon>["name"]; from?: string; to?: string }) {
  const colors = name === "plusCircle" ? ["#3ee0a6", "#06946a"] : ["#5aa9ff", "#2058d8"];
  return <span className="installation-action-icon" aria-hidden="true" style={{"--action-icon-top": from ?? colors[0], "--action-icon-bottom": to ?? colors[1]} as CSSProperties}><MenuIcon name={name}/></span>;
}
