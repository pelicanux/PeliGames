// "Share screen" with the choice of sound, as in Discord. The system's screen
// picker (the Wayland portal) has no audio option, so the sound is chosen here
// first: none, the whole PC except the call, or one application.
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { shareMatrixScreen, useMatrixVoice, type ShareAudio } from "../services/matrixVoice";
import { coverNativeVideo } from "../services/matrixScreenView";

type AudioApp = { key: string; name: string };
const CHOICE_KEY = "peligames.matrixShareAudio";
function savedChoice(): ShareAudio {
  try {
    const saved = JSON.parse(localStorage.getItem(CHOICE_KEY) || "null") as ShareAudio | null;
    if (saved?.kind === "none" || saved?.kind === "all") return saved;
    if (saved?.kind === "app" && typeof saved.key === "string" && saved.key.length <= 256) return saved;
  } catch { /* The default below. */ }
  return { kind: "all" };
}

export function MatrixShareButton({ pt, icon }: { pt: boolean; icon: React.ReactNode }) {
  const voice = useMatrixVoice();
  const [open, setOpen] = useState(false);
  const [choice, setChoice] = useState<ShareAudio>(savedChoice);
  const [apps, setApps] = useState<AudioApp[] | null>(null);
  const box = useRef<HTMLDivElement>(null);
  const connected = voice.phase === "connected";
  const loadApps = () => { setApps(null); void invoke<AudioApp[]>("matrix_screen_audio_apps").then(setApps).catch(() => setApps([])); };
  useEffect(() => {
    if (!open) return;
    loadApps();
    const uncover = coverNativeVideo();
    const outside = (event: PointerEvent) => { if (!box.current?.contains(event.target as Node)) setOpen(false); };
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape") setOpen(false); };
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => { uncover(); document.removeEventListener("pointerdown", outside); document.removeEventListener("keydown", escape); };
  }, [open]);
  const pick = (next: ShareAudio) => {
    setChoice(next);
    try { localStorage.setItem(CHOICE_KEY, JSON.stringify(next)); } catch { /* Kept for this session only. */ }
  };
  const start = () => { setOpen(false); void shareMatrixScreen(choice); };
  const same = (a: ShareAudio, b: ShareAudio) => a.kind === b.kind && (a.kind !== "app" || (b.kind === "app" && a.key === b.key));
  // A remembered application that is not playing now still shows, so the choice is visible.
  const listed = apps && choice.kind === "app" && !apps.some(app => app.key === choice.key) ? [...apps, { key: choice.key, name: choice.key }] : apps;
  const option = (value: ShareAudio, label: string, hint?: string) => <label key={value.kind === "app" ? `app:${value.key}` : value.kind} className={same(choice, value) ? "is-chosen" : undefined}>
    <input type="radio" name="matrix-share-audio" checked={same(choice, value)} onChange={() => pick(value)}/>
    <span>{label}{hint && <small>{hint}</small>}</span>
  </label>;
  if (voice.screen !== "off") {
    return <button type="button" className="matrix-call-share is-live" aria-pressed="true" disabled={voice.screen === "choosing"} onClick={() => void shareMatrixScreen(null)}>
      {icon}{voice.screen === "choosing" ? (pt ? "escolhendo tela…" : "choosing screen…") : (pt ? "parar transmissão" : "stop sharing")}</button>;
  }
  return <div ref={box} className="matrix-share-anchor">
    <button type="button" className="matrix-call-share" aria-haspopup="dialog" aria-expanded={open} disabled={!connected} onClick={() => setOpen(value => !value)}>{icon}{pt ? "transmitir tela" : "share screen"}</button>
    {open && <div className="matrix-share-menu" role="dialog" aria-label={pt ? "Som da transmissão" : "Stream sound"}>
      <strong>{pt ? "> som da transmissão" : "> stream sound"}</strong>
      {option({ kind: "all" }, pt ? "Todo o som do PC" : "All PC sound", pt ? "menos a própria chamada" : "except the call itself")}
      {option({ kind: "none" }, pt ? "Sem som" : "No sound")}
      <div className="matrix-share-apps-heading"><span>{pt ? "Só um aplicativo" : "One application only"}</span><button type="button" onClick={loadApps}>{pt ? "[atualizar]" : "[refresh]"}</button></div>
      <div className="matrix-share-apps">
        {listed === null ? <small>{pt ? "procurando…" : "looking…"}</small>
          : listed.length ? listed.map(app => option({ kind: "app", key: app.key }, app.name))
          : <small>{pt ? "Nenhum aplicativo tocando som agora. Abra o jogo e toque em [atualizar]." : "No application playing sound now. Open the game and press [refresh]."}</small>}
      </div>
      <button type="button" className="matrix-share-start" onClick={start}>{pt ? "Escolher tela e transmitir" : "Choose screen and share"}</button>
      <small className="matrix-share-note">{pt ? "Em seguida o sistema pergunta qual tela ou janela mostrar." : "The system then asks which screen or window to show."}</small>
    </div>}
  </div>;
}
