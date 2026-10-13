import { useId } from "react";
import { motion } from "framer-motion";

const cracks = [
  "M0 18 17 23 27 39 42 44 49 61 63 66 72 83 100 91",
  "M27 39 19 53 23 70 10 81 0 83", "M42 44 54 32 71 35 84 19 100 15",
  "M49 61 37 76 43 91 39 100", "M63 66 76 57 91 61 100 54",
  "M17 23 24 9 19 0", "M54 32 48 17 61 8 66 0", "M72 83 88 80 95 100",
  "M19 53 8 49 0 55", "M71 35 73 45 76 57", "M37 76 32 72 23 70",
  "M48 17 39 18 35 10", "M91 61 89 71 94 75", "M84 19 80 11 85 5",
];
const crystals = [[8,14],[88,9],[12,78],[84,87],[6,46],[94,48],[44,5],[54,95]];

export function CoverFreeze({ phase = "hold", reducedMotion = false }: { phase?: "hold" | "frozen" | "melt"; reducedMotion?: boolean }) {
  const id = useId().replace(/:/g, "");
  if (reducedMotion) return phase === "hold" ? <div className="library-hold-progress" aria-hidden="true"><span /></div> : null;
  return <svg className={`cover-freeze cover-freeze--${phase}`} viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
    <defs>
      <radialGradient id={`${id}-ice`}><stop stopColor="#d9f8ff" stopOpacity={phase === "hold" ? .55 : .82}/><stop offset=".65" stopColor="#9dddef" stopOpacity=".88"/><stop offset="1" stopColor="#efffff" stopOpacity=".98"/></radialGradient>
      <mask id={`${id}-freeze`} maskUnits="userSpaceOnUse" x="0" y="0" width="100" height="100">
        <rect width="100" height="100" fill="white"/>
        {phase === "hold" ? <circle className="cover-freeze-gap" cx="50" cy="50" r="75" fill="black"/> :
          <motion.circle className="cover-freeze-thaw-gap" cx="50" cy="50" fill="black" initial={{r:0}} animate={{r:phase === "melt" ? 75 : 0}} transition={{duration:2,delay:phase === "melt" ? .1 : 0,ease:"easeInOut"}}/>}
      </mask>
    </defs>
    <g mask={`url(#${id}-freeze)`}>
      <rect width="100" height="100" fill={`url(#${id}-ice)`}/>
      <image href="/images/button-facets.svg" width="100" height="100" preserveAspectRatio="none" opacity=".55"/>
      <g className="cover-freeze-crystals" fill="none" stroke="#f2ffff" strokeWidth=".65">
        {crystals.map(([x,y]) => <path key={`${x}-${y}`} d={`M${x-7} ${y}h14M${x} ${y-9}v18M${x-5} ${y-6}l10 12M${x-5} ${y+6}l10-12M${x-3} ${y-7}l3 3 3-3M${x-3} ${y+7}l3-3 3 3`}/>) }
      </g>
    </g>
    <g className="cover-freeze-cracks" fill="none" strokeLinejoin="round" strokeLinecap="round">
      {cracks.map((d,index) => <g key={d} style={{ animationDelay:`${.66 + index * .015}s` }}><path className="cover-freeze-crack-shadow" d={d} pathLength="1"/><path d={d} pathLength="1"/></g>)}
    </g>
    {phase === "melt" && <g className="cover-freeze-drops" fill="#c6f3ff">{[20,43,67,86].map((x,index) => <path key={x} style={{animationDelay:`${.45+index*.055}s`}} d={`M${x} 72q-3 6-3 9a3 3 0 0 0 6 0q0-3-3-9Z`}/>)}</g>}
  </svg>;
}
