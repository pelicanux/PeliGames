export function CoverWaterFill({ reducedMotion }: { reducedMotion: boolean }) {
  return <div className={`cover-water-fill${reducedMotion ? " cover-water-fill--reduced" : ""}`} aria-hidden="true">
    <div className="cover-water-level">
      <svg className="cover-water-wave" viewBox="0 0 200 20" preserveAspectRatio="none"><path d="M0 10Q25 0 50 10T100 10T150 10T200 10V20H0Z" /></svg>
      <i /><i /><i />
    </div>
  </div>;
}
