import { MenuIcon } from "../MenuIcon";
import type { ComponentProps } from "react";

/** Spotlight icons match the library prototype without changing Legacy icons. */
export function ModernQuickIcon({name}: ComponentProps<typeof MenuIcon>) {
  const drawing = name === "heart" ? <path fill="currentColor" stroke="none" d="M12 21s-7.5-4.6-10-9.2C.4 8.6 2.2 4 6.5 4c2.3 0 3.9 1.3 5.5 3.2C13.6 5.3 15.2 4 17.5 4 21.8 4 23.6 8.6 22 11.8 19.5 16.4 12 21 12 21z"/>
    : name === "folder" ? <path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>
    : name === "logs" ? <><path d="M6 3h9l4 4v14H6z"/><path d="M9 12h7M9 16h7"/></> : null;
  return drawing ? <svg className="menu-icon" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{drawing}</svg> : <MenuIcon name={name}/>;
}
