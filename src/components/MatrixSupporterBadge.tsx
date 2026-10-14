import { useI18n } from "../i18n/I18nContext";

// Original pixel alien, shared by both supporter categories.
const pixels = [
  "00100000100",
  "00010001000",
  "00111111100",
  "01101110110",
  "11111111111",
  "10111111101",
  "10100000101",
  "00011011000",
];
export function MatrixSupporterBadge() {
  const { language } = useI18n();
  const label = language === "pt" ? "Apoiador · membro do canal ou impulsionador do Discord" : "Supporter · channel member or Discord server booster";
  return <span className="matrix-supporter-badge" role="img" aria-label={label} title={label}>
    <svg viewBox="0 0 11 8" fill="currentColor" shapeRendering="crispEdges" aria-hidden="true">
      {pixels.flatMap((row,y) => [...row].flatMap((pixel,x) => pixel === "1" ? [<rect key={`${x}-${y}`} x={x} y={y} width="1" height="1" />] : []))}
    </svg>
  </span>;
}
