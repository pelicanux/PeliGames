import type { CSSProperties } from "react";

const sparks = Array.from({ length: 18 }, (_, index) => ({
  left: (index % 3 + .5) * 100 / 3,
  top: (Math.floor(index / 3) + .5) * 100 / 6,
  delay: (index * 7 % 18) / 18 * .75,
  dx: (index * 31 % 100) - 50,
  dy: -(20 + index * 13 % 70),
}));

export function CoverHoldParticles({ reducedMotion }: { reducedMotion: boolean }) {
  return <div className={`cover-hold-particles${reducedMotion ? " cover-hold-particles--reduced" : ""}`} aria-hidden="true">
    {sparks.map((spark, index) => <i key={index} style={{ left: `${spark.left}%`, top: `${spark.top}%`, animationDelay: `${spark.delay}s`, "--particle-dx": `${spark.dx}px`, "--particle-dy": `${spark.dy}px` } as CSSProperties} />)}
  </div>;
}
