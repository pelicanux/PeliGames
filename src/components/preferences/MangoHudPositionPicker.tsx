const positions = [
  { value: "top-left", label: "Superior esquerdo", row: 1, column: 1 },
  { value: "top-center", label: "Superior central", row: 1, column: 2 },
  { value: "top-right", label: "Superior direito", row: 1, column: 3 },
  { value: "middle-left", label: "Centro esquerdo", row: 2, column: 1 },
  { value: "middle-right", label: "Centro direito", row: 2, column: 3 },
  { value: "bottom-left", label: "Inferior esquerdo", row: 3, column: 1 },
  { value: "bottom-center", label: "Inferior central", row: 3, column: 2 },
  { value: "bottom-right", label: "Inferior direito", row: 3, column: 3 },
];

export function MangoHudPositionPicker({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  return <div className="mango-position-picker">
    <div className="mango-position-screen" role="group" aria-label="Posição do painel">
      <span className="mango-position-screen-label" aria-hidden="true">HUD</span>
      {positions.map(position => <button key={position.value} type="button" className="mango-position-point"
        style={{ gridRow: position.row, gridColumn: position.column, justifySelf: position.column === 1 ? "start" : position.column === 3 ? "end" : "center", alignSelf: position.row === 1 ? "start" : position.row === 3 ? "end" : "center" }} title={position.label}
        aria-label={position.label} aria-pressed={value === position.value}
        onClick={() => onChange(position.value)}><span /></button>)}
    </div>
    <span className="mango-position-caption">{positions.find(position => position.value === value)?.label}</span>
  </div>;
}
