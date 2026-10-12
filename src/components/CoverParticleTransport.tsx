import { useRef, type CSSProperties } from "react";

// Split the complete card into 24 tiles; only transforms animate between frames.
const tiles = Array.from({ length: 24 }, (_, index) => ({ column: index % 4, row: Math.floor(index / 4), delay: (index * 7 % 11) * .01 }));
export function CoverParticleTransport({ preview, source, target, onComplete }: { preview: HTMLElement; source: DOMRect; target: { left: number; top: number; width: number; height: number }; onComplete: () => void }) {
  const completed = useRef(new Set<number>());
  return <div className="cover-particle-transport" aria-hidden="true">
    {tiles.map((tile, index) => {
      const startX = source.left + tile.column * source.width / 4;
      const startY = source.top + tile.row * source.height / 6;
      const endX = target.left + tile.column * target.width / 4;
      const endY = target.top + tile.row * target.height / 6;
      const tileWidth = target.width / 4 + .3, tileHeight = target.height / 6 + .3;
      return <div key={index} className="cover-launch-particle"
        style={{ width: tileWidth, height: tileHeight, animationDelay: `${tile.delay}s`,
          "--particle-source": `translate3d(${startX}px,${startY}px,0) scale(${(source.width / 4 + .3) / tileWidth},${(source.height / 6 + .3) / tileHeight})`,
          "--particle-scatter": `translate3d(${startX + (index * 37 % 160) - 80}px,${startY - 40 - index * 19 % 120}px,0) rotate(${(index % 2 ? 1 : -1) * 150}deg) scale(${3 / tileWidth},${3 / tileHeight})`,
          "--particle-target": `translate3d(${endX}px,${endY}px,0)`,
        } as CSSProperties}
        onAnimationEnd={event => { if (event.target !== event.currentTarget) return; completed.current.add(index); if (completed.current.size === tiles.length) onComplete(); }}>
        <div className="cover-particle-artwork" style={{ left: -tile.column * target.width / 4, top: -tile.row * target.height / 6, width: target.width, height: target.height }}
          ref={element => {
            if (!element || element.firstChild) return;
            const copy = preview.cloneNode(true) as HTMLElement;
            copy.classList.remove("library-game-card--list");
            copy.style.cssText += ";width:100%;height:100%;box-sizing:border-box;transform:none;position:relative;";
            element.append(copy);
          }} />
      </div>;
    })}
  </div>;
}
