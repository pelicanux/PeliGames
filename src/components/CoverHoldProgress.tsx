/** Hold-to-launch feedback: a light traces the cover's outline while the
 * accent fills in from the edges. Only stroke offset and opacity animate. */
export function CoverHoldProgress() {
  return <div className="library-hold-progress" aria-hidden="true">
    <span className="library-hold-charge" />
    <svg viewBox="0 0 100 150" preserveAspectRatio="none">
      <rect className="library-hold-track" x="1" y="1" width="98" height="148" rx="9" pathLength={1} />
      <rect className="library-hold-glow" x="1" y="1" width="98" height="148" rx="9" pathLength={1} />
      <rect className="library-hold-ring" x="1" y="1" width="98" height="148" rx="9" pathLength={1} />
    </svg>
  </div>;
}
