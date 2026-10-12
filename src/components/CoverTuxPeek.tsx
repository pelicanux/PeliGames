import { useId } from "react";

// Preserve the original eye; the slightly expanded contour covers its white rim.
const originalEye = "M13.033 3.405h.013c.213 0 .396.062.584.198.19.135.33.332.438.533.105.259.158.459.166.724 0-.02.006-.04.006-.06v.105a.086.086 0 01-.004-.021l-.004-.024a1.807 1.807 0 01-.15.706.953.953 0 01-.213.335.71.71 0 00-.088-.042c-.104-.045-.198-.064-.284-.133a1.312 1.312 0 00-.22-.066c.05-.06.146-.133.183-.198.053-.128.082-.264.088-.402v-.02a1.21 1.21 0 00-.061-.4c-.045-.134-.101-.2-.183-.333-.084-.066-.167-.132-.267-.132h-.016c-.093 0-.176.03-.262.132a.8.8 0 00-.205.334 1.18 1.18 0 00-.09.4v.019c.002.089.008.179.02.267-.193-.067-.438-.135-.607-.202a1.635 1.635 0 01-.018-.2v-.02a1.772 1.772 0 01.15-.768c.082-.22.232-.406.43-.533a.985.985 0 01.594-.2z";
export function CoverTuxPeek() {
  const eyeClip = `tux-eye-${useId().replace(/:/g, "")}`;
  return <div className="cover-tux-peek" aria-hidden="true">
    <svg className="cover-tux-head" viewBox="7 0 10 9">
      <defs><clipPath id={eyeClip}><path d={originalEye} /><path d={originalEye} transform="translate(-1.564 -0.546) scale(1.12)" /></clipPath></defs>
      <image href="/icons/tux-color.svg" width="24" height="24" />
      <g clipPath={`url(#${eyeClip})`}>
        <rect className="cover-tux-winking-eye" x="11.9" y="3.3" width="2.5" height="2.8" fill="#141820" />
      </g>
    </svg>
  </div>;
}
