import { ModalSurface } from "./ModalSurface";
import { InstallationActionIcon } from "./InstallationActionIcon";
import "./GameEntryModal.css";
import { useI18n } from "../i18n/I18nContext";
import { launcherRecipes, type LauncherRecipe } from "../services/launcherInstallation";
import { LauncherInstallWizard } from "./LauncherInstallWizard";
import { useEffect, useState } from "react";

export function GameEntryModal({ onClose, onSelect }: { onClose: () => void; onSelect: (mode: "install" | "add") => void }) {

  const { t } = useI18n();
  const [recipe, setRecipe] = useState<LauncherRecipe | null>(null);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape" && !recipe) { event.stopImmediatePropagation(); onClose(); } };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [onClose, recipe]);
  if (recipe) return <LauncherInstallWizard recipe={recipe} onClose={() => setRecipe(null)} />;
  return <div className="modal-overlay" onClick={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass mod-manager-modal modern-dialog game-entry-dialog" role="dialog" aria-modal="true" aria-labelledby="game-entry-title"
      onDismiss={onClose} header={<h2 id="game-entry-title">{t("gameModes", "entryTitle")}</h2>}>
      <div className="mod-manager-options game-entry-options">
        {(["install", "add"] as const).map(mode => <button key={mode} type="button" className="game-entry-option"
          onClick={() => onSelect(mode)}><InstallationActionIcon name={mode === "install" ? "download" : "plusCircle"} /><span>{t("gameModes", mode === "install" ? "install" : "addGame")}</span></button>)}
      </div>
      <section className="game-entry-launchers" aria-label={t("ui-game-entry-modal", "install_launcher")}>
        <h3>{t("ui-game-entry-modal", "install_launcher")}</h3>
        <div className="game-entry-launcher-grid">{launcherRecipes.map(item => <button type="button" key={item.id} className="game-entry-launcher" aria-label={`${t("ui-game-entry-modal", "install")} ${item.name}`} onClick={() => setRecipe(item)}><img src={`/launchers/${item.id}.svg`} alt="" /><span>{item.name}</span></button>)}</div>
      </section>
    </ModalSurface>
  </div>;
}
