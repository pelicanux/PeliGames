import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { motion } from "framer-motion";
import { useI18n } from "../i18n/I18nContext";
import type { GameInfo } from "./GameGrid";
import { CoverTuxPeek } from "./CoverTuxPeek";
import { CoverParticleTransport } from "./CoverParticleTransport";

export interface CoverTeleport { game: GameInfo; source: DOMRect; preview: HTMLElement; effect: "shatter" | "smooth" | "aquarium" | "particles" | "penguin" | "shrink"; }
// Shared irregular vertices keep 24 triangular fragments fitted without gaps.
const points = Array.from({ length: 5 }, (_, row) => Array.from({ length: 4 }, (_, column) => ({
  x: column * 100 / 3 + (column > 0 && column < 3 ? ((row * 11 + column * 17) % 9) - 4 : 0),
  y: row * 25 + (row > 0 && row < 4 ? ((row * 13 + column * 7) % 9) - 4 : 0),
})));
const pieces = Array.from({ length: 12 }, (_, cell) => {
  const row = Math.floor(cell / 3), column = cell % 3;
  const a = points[row][column], b = points[row][column + 1];
  const c = points[row + 1][column], d = points[row + 1][column + 1];
  return (cell % 2 ? [[a, b, c], [b, d, c]] : [[a, b, d], [a, d, c]]).map(vertices => ({
    vertices,
    clipPath: `polygon(${vertices.map(point => `${point.x}% ${point.y}%`).join(",")})`,
    x: vertices.reduce((sum, point) => sum + point.x, 0) / 3,
    y: vertices.reduce((sum, point) => sum + point.y, 0) / 3,
  }));
}).flat();
const crackEdges = new Map<string, { d: string; distance: number }>();
for (const piece of pieces) for (let index = 0; index < 3; index++) {
  const a = piece.vertices[index], b = piece.vertices[(index + 1) % 3];
  if ((a.x === b.x && (a.x === 0 || a.x === 100)) || (a.y === b.y && (a.y === 0 || a.y === 100))) continue;
  const key = [`${a.x},${a.y}`, `${b.x},${b.y}`].sort().join("|");
  crackEdges.set(key, { d: `M${a.x} ${a.y}L${b.x} ${b.y}`, distance: Math.hypot((a.x + b.x) / 2 - 50, (a.y + b.y) / 2 - 50) });
}
export const coverCrackPaths = [...crackEdges.values()].sort((a, b) => a.distance - b.distance).map(edge => edge.d);
export function CoverLaunchTeleport({ value, reducedMotion, onCancel, cancellingDisabled }: { value: CoverTeleport; reducedMotion: boolean; onCancel: () => void; cancellingDisabled: boolean }) {
  const { language } = useI18n();
  const [assembled, setAssembled] = useState(reducedMotion);
  const [viewport, setViewport] = useState({ width: window.innerWidth, height: window.innerHeight });
  const destination = useRef<HTMLDivElement>(null);
  const completedShards = useRef(new Set<number>());
  const height = Math.min(380, viewport.height * .52), width = height * 2 / 3;
  const target = { left: (viewport.width - width) / 2, top: Math.max(60, (viewport.height - height) / 2 - 84), width, height };
  const waterSource = { x: value.source.left + value.source.width / 2, y: value.source.top + value.source.height * .65 };
  const waterEnd = { x: target.left + width / 2, y: target.top + height };
  const waterPath = `M${waterSource.x} ${waterSource.y} C${waterSource.x} ${Math.min(waterSource.y, target.top) - 80},${waterEnd.x} ${target.top - 100},${waterEnd.x} ${waterEnd.y}`;
  const clone = () => {
    const copy = value.preview.cloneNode(true) as HTMLElement;
    copy.classList.remove("library-game-card--list");
    copy.style.cssText += ";width:100%;height:100%;box-sizing:border-box;transform:none;position:relative;";
    return copy;
  };
  useLayoutEffect(() => { destination.current?.replaceChildren(clone()); }, [value]);
  useEffect(() => {
    const resize = () => setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", resize);
    if (reducedMotion) setAssembled(true);
    return () => { window.removeEventListener("resize", resize); };
  }, [reducedMotion, value]);
  return createPortal(<motion.div className="cover-launch-overlay" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: .18 }}>
    {!assembled && !reducedMotion && value.effect === "particles" && <CoverParticleTransport preview={value.preview} source={value.source} target={target} onComplete={() => setAssembled(true)} />}
    {!assembled && value.effect === "shatter" && pieces.map((piece, index) => <motion.div key={index} className="cover-launch-shard" style={{ clipPath: piece.clipPath, transformOrigin: `${piece.x}% ${piece.y}%` }}
      initial={{ left: value.source.left, top: value.source.top, width: value.source.width, height: value.source.height, rotate: 0 }}
      animate={{ left: [value.source.left, value.source.left + (piece.x - 50) * 1.15, target.left], top: [value.source.top, value.source.top + (piece.y - 50) * 1.1, target.top], width: [value.source.width, value.source.width, target.width], height: [value.source.height, value.source.height, target.height], rotate: [0, (index % 2 ? 1 : -1) * (20 + index % 5 * 6), 0], opacity: [1, .9, 1] }}
      transition={{ duration: .95, delay: (index % 5) * .015, times: [0, .23, 1], ease: [.22, 1, .36, 1] }}
      onAnimationComplete={() => {
        completedShards.current.add(index);
        if (completedShards.current.size === pieces.length) setAssembled(true);
      }}
      ref={element => { if (element && !element.firstChild) element.append(clone()); }} />)}
    {!assembled && !reducedMotion && value.effect === "aquarium" && <svg className="cover-waterfall" width={viewport.width} height={viewport.height} aria-hidden="true">
      <motion.path d={waterPath} className="cover-waterfall-stream" initial={{ pathLength: 0, opacity: 0 }} animate={{ pathLength: [0, 1, 1], opacity: [0, .9, 0] }} transition={{ duration: 1.05, times: [0, .45, 1] }} />
      <motion.path d={waterPath} className="cover-waterfall-glint" initial={{ pathLength: 0, opacity: 0 }} animate={{ pathLength: [0, 1, 1], opacity: [0, .8, 0] }} transition={{ duration: 1.05, times: [0, .45, 1] }} />
      <motion.ellipse cx={waterEnd.x} cy={waterEnd.y} rx={width * .65} ry={18} className="cover-waterfall-ripple" initial={{ opacity: 0, scale: .2 }} animate={{ opacity: [0, .85, 0], scale: [.2, 1, 1.2] }} transition={{ duration: 1.05, times: [0, .6, 1] }} />
    </svg>}
    {value.effect === "shrink" ? <motion.div className="cover-launch-remounted is-assembled" style={target}
      initial={{ scale: reducedMotion ? 1 : 0 }} animate={{ scale: 1 }}
      transition={{ duration: reducedMotion ? 0 : .5, ease: [.22, 1, .36, 1] }}
      onAnimationComplete={() => setAssembled(true)} ref={destination} aria-hidden="true" />
      : value.effect === "aquarium" ? <motion.div className="cover-launch-remounted cover-launch-remounted--shatter is-assembled" style={target}
      initial={{ clipPath: reducedMotion ? "inset(0% 0 0 0)" : "inset(100% 0 0 0)" }}
      animate={{ clipPath: reducedMotion ? "inset(0% 0 0 0)" : ["inset(100% 0 0 0)", "inset(100% 0 0 0)", "inset(0% 0 0 0)"] }}
      transition={{ duration: reducedMotion ? 0 : 1.05, times: [0, .3, 1], ease: "easeOut" }}
      onAnimationComplete={() => setAssembled(true)} ref={destination} aria-hidden="true" />
      : (value.effect === "smooth" || value.effect === "penguin") ? <motion.div className="cover-launch-remounted is-assembled"
      initial={reducedMotion ? target : { left: value.source.left, top: value.source.top, width: value.source.width, height: value.source.height }}
      animate={target} transition={{ duration: reducedMotion ? 0 : .5, ease: [.22, 1, .36, 1] }}
      onAnimationComplete={() => setAssembled(true)} ref={destination} aria-hidden="true" />
      : <div className={`cover-launch-remounted cover-launch-remounted--shatter${assembled ? " is-assembled" : ""}`} style={target} ref={destination} aria-hidden="true" />}
    {assembled && value.effect === "penguin" && !reducedMotion && <div className="cover-tux-peek-frame" style={target}><CoverTuxPeek /></div>}
    <motion.div className="cover-launch-message" style={{ top: target.top + height + 20 }} initial={{ opacity: 0 }} animate={{ opacity: assembled ? 1 : 0 }} transition={{ duration: .2 }} role="status" aria-live="polite">
      <div className="nexus-loading-emblem" aria-hidden="true"><span className="nexus-loading-ring" /><img src="/peligames.svg" alt="" /></div>
      <div><strong>{language === "en" ? "Starting game…" : "Iniciando jogo…"}</strong><span>{value.game.name}</span></div>
    </motion.div>
    {assembled && <div className="cover-launch-cancel" style={{ top: target.top + height + 84 }}>
      <button type="button" className="button-secondary" disabled={cancellingDisabled} onClick={onCancel}>{language === "en" ? "Cancel" : "Cancelar"}</button>
    </div>}
  </motion.div>, document.body);
}
