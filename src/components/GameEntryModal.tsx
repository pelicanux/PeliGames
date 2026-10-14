import { ModalSurface } from "./ModalSurface";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { launcherRecipes, type LauncherRecipe } from "../services/launcherInstallation";
import { LauncherInstallWizard } from "./LauncherInstallWizard";
import { useEffect, useState } from "react";

export function GameEntryModal({ onClose, onSelect }: { onClose: () => void; onSelect: (mode: "install" | "add") => void }) {
  const { t, language } = useI18n();
  const [recipe, setRecipe] = useState<LauncherRecipe | null>(null);
  useEffect(() => {
    const escape = (event: KeyboardEvent) => { if (event.key === "Escape" && !recipe) { event.stopImmediatePropagation(); onClose(); } };
    window.addEventListener("keydown", escape, true);
    return () => window.removeEventListener("keydown", escape, true);
  }, [onClose, recipe]);
  if (recipe) return <LauncherInstallWizard recipe={recipe} onClose={() => setRecipe(null)} />;
  return <div className="modal-overlay" onClick={event => { if (event.target === event.currentTarget) onClose(); }}>
    <ModalSurface className="modal-content dialog-glass mod-manager-modal" role="dialog" aria-modal="true" aria-labelledby="game-entry-title"
      onDismiss={onClose} header={<h2 id="game-entry-title">{t("gameModes", "entryTitle")}</h2>}>
      <div className="mod-manager-options game-entry-options">
        {(["install", "add"] as const).map(mode => <button key={mode} type="button" className="btn btn-secondary game-entry-option"
          onClick={() => onSelect(mode)}><MenuIcon name={mode === "install" ? "download" : "folder"} /><span>{t("gameModes", mode === "install" ? "install" : "addGame")}</span></button>)}
      </div>
      <section className="game-entry-launchers" aria-label={language === "pt" ? "Instalar launcher" : "Install launcher"}>
        <h3>{language === "pt" ? "Instalar launcher" : "Install launcher"}</h3>
        <div className="game-entry-launcher-grid">{launcherRecipes.map(item => <button type="button" key={item.id} className="game-entry-launcher" aria-label={`${language === "pt" ? "Instalar" : "Install"} ${item.name}`} onClick={() => setRecipe(item)}><img src={`/launchers/${item.id}.svg`} alt="" /><span>{item.name}</span></button>)}</div>
      </section>
    </ModalSurface>
  </div>;
}
