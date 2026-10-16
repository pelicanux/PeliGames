// Tux winks: a lid in the head's colour slides down over the right eye (measured
// from tux-color.svg: white spans x 11.86–14.22, y 3.41–5.88), the beak is drawn
// again on top so the lid never covers it, and a closed-eye line shows while shut.
const beak = "M9.4 6.1Q11.5 4.4 13.4 5.8L14.6 6.5Q14.1 7.4 12 8Q10.8 8.4 9.5 7.2L8.9 6.7Z";
const beakLine = "M9.8 6.8Q11.5 7.8 13.7 6.6";

/** `rise`: the head comes up from the corner while the hold charges.
 * `wink`: already shown, it winks and then goes back down. */
export function CoverTuxPeek({ mode }: { mode: "rise" | "wink" }) {
  return <div className={`cover-tux-peek cover-tux-peek--${mode}`} aria-hidden="true">
    <svg className="cover-tux-head" viewBox="7 0 10 9">
      <image href="/icons/tux-color.svg" width="24" height="24" />
      <ellipse className="cover-tux-lid" cx="13.04" cy="4.7" rx="1.46" ry="1.5" fill="#141820" />
      <path className="cover-tux-lash" d="M12.02 4.86Q13.04 5.5 14.06 4.86" fill="none" stroke="#fafaf5" strokeWidth=".24" strokeLinecap="round" />
      <path d={beak} fill="#ffc02b" stroke="#8e4d0d" strokeWidth=".25" />
      <path d={beakLine} fill="none" stroke="#99590e" strokeWidth=".2" />
    </svg>
  </div>;
}
