import type { ReactNode } from "react";

type IconName = "copy" | "discord" | "eye" | "language" | "image" | "tools" | "gamepad" | "gamepadSearch" | "heart" | "wine" | "search" | "info" | "home" | "plus" | "plusCircle" | "back" | "play" | "trash" | "folderSearch" | "edit" | "check" | "folder" | "calendar" | "platform" | "chip" | "graphics" | "puzzle" | "logs" | "document" | "settings" | "cube" | "keyboard" | "download" | "repair" | "bolt" | "neural";
const drawings: Record<IconName, ReactNode> = {
  copy: <><rect x="8" y="8" width="13" height="13" rx="2"/><path d="M16 8V5a2 2 0 0 0-2-2H5a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h3"/></>,
  discord: <path fill="currentColor" stroke="none" d="M19.27 5.33C17.94 4.71 16.5 4.26 15 4a.09.09 0 0 0-.07.03c-.18.33-.39.76-.53 1.09a16.09 16.09 0 0 0-4.8 0c-.14-.33-.35-.76-.53-1.09a.09.09 0 0 0-.07-.03c-1.5.26-2.93.71-4.27 1.33-.01 0-.02.01-.03.02-2.72 4.07-3.47 8.03-3.1 11.95 0 .02.01.04.03.05 1.8 1.32 3.53 2.12 5.24 2.65.03.01.06 0 .07-.02.4-.55.76-1.13 1.07-1.74.02-.04 0-.08-.04-.09-.57-.22-1.11-.48-1.64-.78-.04-.02-.04-.08-.01-.11.11-.08.22-.17.33-.25.02-.02.05-.02.07-.01 3.44 1.57 7.15 1.57 10.55 0 .02-.01.05-.01.07.01.11.09.22.17.33.26.03.03.03.09-.01.11-.52.31-1.07.56-1.64.78-.04.01-.05.06-.04.09.32.61.68 1.19 1.07 1.74.01.02.04.03.07.02 1.71-.53 3.44-1.33 5.24-2.65.02-.01.03-.03.03-.05.44-4.53-.73-8.46-3.1-11.95-.01-.01-.02-.02-.03-.02zM8.52 14.91c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12 0 1.17-.84 2.12-1.89 2.12zm6.97 0c-1.03 0-1.89-.95-1.89-2.12s.84-2.12 1.89-2.12c1.06 0 1.9.96 1.89 2.12 0 1.17-.83 2.12-1.89 2.12z"/>,
  eye: <><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12Z"/><circle cx="12" cy="12" r="3"/></>,
  language: <><circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18"/></>,
  image: <><rect x="3" y="3" width="18" height="18" rx="3"/><circle cx="8" cy="8" r="1.5"/><path d="m3 17 5-5 4 4 4-6 5 7"/></>,
  tools: <path d="M14 6a5 5 0 0 0-6 6L3 17a3 3 0 0 0 4 4l5-5a5 5 0 0 0 6-6l-3 3-4-4 3-3Z"/>,
  gamepad: <><path d="M7 6h10c2 0 3 2 4 6l1 6c0 2-2 3-3 1l-3-3H8l-3 3c-1 2-3 1-3-1l1-6c1-4 2-6 4-6Z"/><path d="M7 9v5M4.5 11.5h5M16 10h.01M19 13h.01"/></>,
  gamepadSearch: <path fill="currentColor" stroke="none" fillRule="evenodd" d="M7 5.5h10c2.5 0 3.7 2.2 4.5 6l1 5.5c.5 2.8-2.2 4-4 2l-3-3h-7l-3 3c-1.8 2-4.5.8-4-2l1-5.5C3.3 7.7 4.5 5.5 7 5.5ZM14.8 10.5a3.8 3.8 0 1 0-6.7 2.4 3.8 3.8 0 0 0 6.7-2.4Zm-1.6 0a2.2 2.2 0 1 1-4.4 0 2.2 2.2 0 0 1 4.4 0Zm.1 3.4 1.1-1.1 2.9 2.9-1.1 1.1-2.9-2.9Z" />,
  heart: <path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0L12 5.7l-1.1-1.1a5.5 5.5 0 0 0-7.8 7.8L12 21l8.8-8.6a5.5 5.5 0 0 0 0-7.8Z" />,
  wine: <><path d="M7 3h10l1 6a6 6 0 0 1-12 0l1-6ZM12 15v6M8 21h8M6 8h12" /></>,
  search: <><circle cx="10.5" cy="10.5" r="6.5"/><path d="m16 16 5 5"/></>,
  info: <><circle cx="12" cy="12" r="9"/><path d="M12 11v6m0-11v1"/></>,
  home: <><path d="m3 10 9-7 9 7v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V10Z"/><path d="M9 21v-8h6v8"/></>,
  plus: <path d="M12 5v14M5 12h14" />,
  plusCircle: <><circle cx="12" cy="12" r="9"/><path d="M12 8v8M8 12h8"/></>,
  back: <path d="M19 12H5m7-7-7 7 7 7" />,
  play: <path d="m8 5 11 7-11 7V5Z" />,
  trash: <><path d="M3 6h18M9 6V4h6v2M5 6l1 14h12l1-14M10 10v6m4-6v6" /></>,
  folderSearch: <><path d="M20 8V6a2 2 0 0 0-2-2h-6l-2-2H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h6"/><circle cx="15" cy="14" r="4"/><path d="m18 17 4 4"/></>,
  edit: <path d="M17 3a2.828 2.828 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5L17 3Z" />,
  check: <path d="m5 12 4 4L19 6" />,
  neural: <><circle cx="5" cy="5" r="2"/><circle cx="19" cy="5" r="2"/><circle cx="12" cy="12" r="3"/><circle cx="5" cy="19" r="2"/><circle cx="19" cy="19" r="2"/><path d="m6.5 6.5 3.4 3.4m4.2 0 3.4-3.4m-7.6 7.6-3.4 3.4m7.6-3.4 3.4 3.4"/></>,
  bolt: <path d="m13 2-9 12h7l-1 8 10-13h-7l0-7Z"/>,
  folder: <><path d="M3 7V5a2 2 0 0 1 2-2h4l2 3h8a2 2 0 0 1 2 2v2"/><path d="M3 8h17a1 1 0 0 1 1 1l-2 10H3a1 1 0 0 1-1-1L3 8Z"/></>,
  calendar: <><rect x="3" y="5" width="18" height="16" rx="2"/><path d="M8 3v4m8-4v4M3 11h18M8 15h2m4 0h2M8 18h2m4 0h2"/></>,
  platform: <><rect x="3" y="3" width="18" height="14" rx="2"/><path d="M8 21h8m-4-4v4"/></>,
  chip: <><rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9" y="9" width="6" height="6" rx="1"/><path d="M9 3v3m6-3v3M9 18v3m6-3v3M3 9h3m-3 6h3m12-6h3m-3 6h3"/></>,
  graphics: <><rect x="5" y="6" width="16" height="12" rx="2"/><circle cx="13" cy="12" r="3"/><path d="M2 5v14h3M8 18v3h8v-3"/></>,
  puzzle: <path d="M6 5h4V4a2 2 0 0 1 4 0v1h4a1 1 0 0 1 1 1v4h1a2 2 0 0 1 0 4h-1v4a1 1 0 0 1-1 1h-4v-1a2 2 0 0 0-4 0v1H6a1 1 0 0 1-1-1v-4h1a2 2 0 0 0 0-4H5V6a1 1 0 0 1 1-1Z"/>,
  logs: <><rect x="4" y="3" width="16" height="18" rx="2"/><path d="M8 7h8M8 11h8M8 15h5M8 18h8"/></>,
  document: <><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9Z"/><path d="M14 3v6h6M8 13h8M8 17h6"/></>,
  settings: <><path d="M9.5 3h5l.6 2.4 1.6.9 2.4-.7 2.5 4.3-1.8 1.7v1.8l1.8 1.7-2.5 4.3-2.4-.7-1.6.9-.6 2.4h-5l-.6-2.4-1.6-.9-2.4.7-2.5-4.3 1.8-1.7v-1.8L2.4 9.9l2.5-4.3 2.4.7 1.6-.9L9.5 3Z"/><circle cx="12" cy="12" r="3"/></>,
  cube: <><path d="m12 2 9 5v10l-9 5-9-5V7l9-5Z"/><path d="m3 7 9 5 9-5m-9 5v10M7.5 4.5l9 5"/></>,
  keyboard: <><rect x="2" y="5" width="20" height="14" rx="2"/><path d="M6 9h.01M10 9h.01M14 9h.01M18 9h.01M6 12h.01M10 12h.01M14 12h.01M18 12h.01M7 16h10"/></>,
  download: <><path d="M12 3v12m-5-5 5 5 5-5M4 15v5h16v-5"/></>,
  repair: <><path d="M3 11a9 9 0 0 1 15.36-5.36L21 8M21 13a9 9 0 0 1-15.36 5.36L3 16"/><path d="M21 3v5h-5M3 21v-5h5"/></>,
};

export function MenuIcon({ name }: { name: IconName }) {
  return <svg className="menu-icon" width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={name === "repair" ? 2 : 1.8} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true" focusable="false">{drawings[name]}</svg>;
}
