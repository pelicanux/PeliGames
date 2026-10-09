import { NexusLibraryLoading } from "./NexusLibraryLoading";
import { motion } from "framer-motion";
import type { MouseEventHandler } from "react";
import { MenuIcon } from "../../components/MenuIcon";
import { useNexusText } from "./text";
export function NexusEmptySummary({ onAdd, onCollapse, busy, loaded, reducedMotion, onCoverMove, onCoverLeave }: {
  onAdd: () => void; onCollapse: () => void; busy: boolean; loaded: boolean; reducedMotion: boolean;
  onCoverMove: MouseEventHandler<HTMLDivElement>; onCoverLeave: MouseEventHandler<HTMLDivElement>;
}) {
  const text = useNexusText();
  return <div className="game-summary game-install-summary"><div className="game-cover-column"><motion.div className="selected-cover installation-cover" onMouseMove={onCoverMove} onMouseLeave={onCoverLeave} layout={reducedMotion ? false : "preserve-aspect"}><div className="glare" /><div className="installation-cover-placeholder"><img src="/nexus-mods.svg" alt="" /></div></motion.div><button type="button" className="btn game-back-button installation-collapse" onClick={onCollapse}><MenuIcon name="back" />{text.mods}</button></div><div className="game-action-column"><h3 className="selected-game-title">{text.title}</h3>{busy || !loaded ? <NexusLibraryLoading reducedMotion={reducedMotion} /> : <><p className="nexus-hint">{text.emptyHint}</p><button type="button" className="btn btn-primary" disabled={busy} onClick={onAdd}><MenuIcon name="plusCircle" />{text.add}</button></>}</div></div>;
}
