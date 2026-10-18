import { useEffect, useState } from "react";
import { loadMangoHudProfile } from "../../services/mangohud";
import { notifyError } from "../../services/notifications";
import { MangoHudBenchmarkComparison, type BenchmarkGame } from "./MangoHudBenchmarkComparison";
import { shortcutLabel } from "./mangoHudShortcut";
import { useI18n } from "../../i18n/I18nContext";
import "./modernMangoHud.css";

export function GameBenchmarkView({ game, onBack }: { game: BenchmarkGame; onBack: () => void }) {
  const { language } = useI18n();
  const [shortcut,setShortcut] = useState("atalho de gravação configurado");
  const [ready,setReady] = useState(false);
  useEffect(() => {
    let active = true;
    void loadMangoHudProfile().then(profile => {
      if (active) setShortcut(shortcutLabel(String(profile.settings.toggle_logging || "Shift_L+F2")));
    }).catch(error => { if (active) notifyError("Benchmarks MangoHUD",error); })
      .finally(() => { if (active) setReady(true); });
    return () => { active = false; };
  }, []);
  return <section className="mango-modern game-benchmark-view">
    <div className="game-benchmark-header">
      <button type="button" autoFocus className="modern-mini-btn" onClick={onBack}>
        {language === "pt" ? "← Voltar ao desempenho" : "← Back to performance"}
      </button>
    </div>
    {ready ? <MangoHudBenchmarkComparison shortcut={shortcut} game={game} fullPage /> : <p role="status">Carregando comparação…</p>}
  </section>;
}
