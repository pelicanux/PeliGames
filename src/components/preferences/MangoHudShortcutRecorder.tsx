import { useEffect, useRef, useState } from "react";
import { mangoHudShortcut, shortcutLabel } from "./mangoHudShortcut";

export function MangoHudShortcutRecorder({ label, value, onChange }: { label: string; value: string; onChange: (value: string) => void }) {
  const [recording, setRecording] = useState(false);
  const [hint, setHint] = useState("");
  const button = useRef<HTMLButtonElement>(null);
  const change = useRef(onChange);
  change.current = onChange;
  useEffect(() => {
    if (!recording) return;
    const held = new Set<string>();
    const down = (event: KeyboardEvent) => {
      event.preventDefault(); event.stopImmediatePropagation();
      if (event.repeat || event.isComposing) return;
      if (event.code === "Escape") { setRecording(false); setHint(""); return; }
      held.add(event.code);
      const shortcut = mangoHudShortcut(event, held);
      if (!shortcut) {
        if (!/^(Shift|Control|Alt|Meta)(Left|Right)$/.test(event.code)) setHint("Tecla não suportada. Tente outra ou pressione Esc para cancelar.");
        return;
      }
      change.current(shortcut); setRecording(false); setHint("");
    };
    const up = (event: KeyboardEvent) => { event.preventDefault(); event.stopImmediatePropagation(); held.delete(event.code); };
    const cancel = () => { setRecording(false); setHint(""); };
    window.addEventListener("keydown", down, true);
    window.addEventListener("keyup", up, true);
    window.addEventListener("blur", cancel);
    return () => { window.removeEventListener("keydown", down, true); window.removeEventListener("keyup", up, true); window.removeEventListener("blur", cancel); };
  }, [recording]);
  return <div className="mm-shortcut-recorder">
    <button ref={button} type="button" className="mm-ghost" data-shortcut-recording={recording || undefined} aria-label={`${label}: ${recording ? "pressione uma tecla" : shortcutLabel(value)}`} aria-pressed={recording}
      onClick={() => { setHint(""); setRecording(active => !active); }} onBlur={() => { setRecording(false); setHint(""); }}>
      {recording ? "Pressione uma tecla…" : shortcutLabel(value)}
    </button>
    <small role="status">{hint || (recording ? "Combine com Ctrl, Alt ou Shift. Esc cancela." : "Clique para gravar outro atalho.")}</small>
  </div>;
}
