import type { CSSProperties } from "react";

// A steady stream of sparks rising off the cover while the hold charges,
// denser toward the bottom; fixed values, so every hold looks the same.
const sparks = Array.from({ length: 32 }, (_, index) => {
  const spread = (index * 37 % 100) / 100;
  return {
    left: 6 + spread * 88,
    top: 30 + (index * 53 % 70),
    size: 4 + index * 7 % 6,
    delay: (index * 29 % 32) / 32 * .9,
    duration: .7 + (index * 11 % 4) * .1,
    dx: ((index * 17 % 40) - 20),
    dy: -(40 + index * 23 % 90),
  };
});

export function CoverHoldParticles({ reducedMotion }: { reducedMotion: boolean }) {
  return <div className={`cover-hold-particles${reducedMotion ? " cover-hold-particles--reduced" : ""}`} aria-hidden="true">
    {sparks.map((spark, index) => <i key={index} style={{ left: `${spark.left}%`, top: `${spark.top}%`, width: spark.size, height: spark.size, animationDelay: `${spark.delay}s`, animationDuration: `${spark.duration}s`, "--particle-dx": `${spark.dx}px`, "--particle-dy": `${spark.dy}px` } as CSSProperties} />)}
  </div>;
}
