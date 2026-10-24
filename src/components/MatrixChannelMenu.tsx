// Right-click menu of a channel, as in Discord, with what Matrix offers.
import { useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNativeVideoOverlay } from "../services/matrixScreenView";

export type ChannelMenuItem = { label: string; hidden?: boolean } & (
  /** A command; `checked` shows it as a toggle, which keeps the menu open. */
  | { action: () => void; danger?: boolean; checked?: boolean; slider?: never }
  /** A percentage slider (a person's volume); changes apply while dragging. */
  | { slider: { value: number; max: number; onChange: (value: number) => void }; action?: never }
);

/** A matrix.to link opens the channel in any Matrix client (Element included). */
export const channelLink = (room: string) => `https://matrix.to/#/${encodeURIComponent(room)}?via=matrix.org`;
export const copyText = (text: string) => invoke("plugin:clipboard-manager|write_text", { text });

export function MatrixChannelMenu({ x, y, title, groups, onClose }: {
  x: number; y: number; title: string;
  /** Item groups, split by a line. */
  groups: ChannelMenuItem[][];
  onClose: () => void;
}) {
  const menu = useRef<HTMLDivElement>(null);
  useNativeVideoOverlay(menu);
  const [place, setPlace] = useState({ left: x, top: y });
  // Kept inside the window, as a system menu.
  useLayoutEffect(() => {
    const box = menu.current?.getBoundingClientRect();
    if (!box) return;
    setPlace({ left: Math.max(4, Math.min(x, window.innerWidth - box.width - 4)), top: Math.max(4, Math.min(y, window.innerHeight - box.height - 4)) });
    menu.current?.querySelector<HTMLButtonElement>("button")?.focus();
  }, [x, y]);
  const visible = groups.map(group => group.filter(item => !item.hidden)).filter(group => group.length);
  return <div className="matrix-channel-menu-layer" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }} onContextMenu={event => { event.preventDefault(); onClose(); }}>
    <div ref={menu} className="matrix-channel-menu" role="menu" aria-label={title} style={place}
      onKeyDown={event => {
        if (event.key === "Escape") { event.stopPropagation(); onClose(); return; }
        if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
        event.preventDefault();
        const buttons = Array.from(menu.current?.querySelectorAll<HTMLElement>("button, input") ?? []);
        const index = buttons.indexOf(document.activeElement as HTMLElement);
        buttons[(index + (event.key === "ArrowDown" ? 1 : buttons.length - 1)) % buttons.length]?.focus();
      }}>
      <strong title={title}>{title}</strong>
      {visible.map((group, index) => <div key={index} className="matrix-channel-menu-group">
        {group.map(item => item.slider ? <label key={item.label} className="matrix-channel-menu-slider">
          <span>{item.label}<b>{item.slider.value}%</b></span>
          <input type="range" min="0" max={item.slider.max} step="5" value={item.slider.value} aria-label={item.label} onChange={event => item.slider.onChange(Number(event.target.value))}
            onKeyDown={event => { if (event.key === "ArrowUp" || event.key === "ArrowDown") event.stopPropagation(); }}/>
        </label> : item.checked !== undefined
          ? <button key={item.label} type="button" role="menuitemcheckbox" aria-checked={item.checked} className="matrix-channel-menu-check" onClick={item.action}>{item.label}<span aria-hidden="true">{item.checked ? "[x]" : "[ ]"}</span></button>
          : <button key={item.label} type="button" role="menuitem" className={item.danger ? "is-danger" : undefined} onClick={() => { onClose(); item.action(); }}>{item.label}</button>)}
      </div>)}
    </div>
  </div>;
}
