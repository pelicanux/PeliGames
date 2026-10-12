/** Keep Tux intact: animate only its movement across the cover. */
export function CoverWalkingPenguin(_props: { reducedMotion: boolean }) {
  return <div className="cover-penguin-walk" aria-hidden="true">
    <div className="cover-penguin-track"><div className="cover-penguin-travel">
      <img src="/icons/tux-color.svg" className="cover-walking-penguin" alt=""  />
    </div></div>
  </div>;
}
