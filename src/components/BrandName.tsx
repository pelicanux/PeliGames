/** Shared wordmark: one font for every letter, split only by color. */
export function BrandName({ draggable = false }: { draggable?: boolean }) {
  return <>Peli<span className="titlebar-games" data-tauri-drag-region={draggable || undefined}>Games</span></>;
}
