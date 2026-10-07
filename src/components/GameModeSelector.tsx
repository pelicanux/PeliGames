import { HoverTooltip } from "./HoverTooltip";
import { useI18n } from "../i18n/I18nContext";
import { MenuIcon } from "./MenuIcon";

export type GamePanelMode = "mods" | "install" | "add" | "installed";

interface Props {
  mode: GamePanelMode;
  open: boolean;
  disabled?: boolean;
  modLibrary: boolean;
  onHome: () => void;
  onChange: (mode: GamePanelMode) => void;
}

export function GameModeSelector({ mode, open, onChange, disabled, modLibrary, onHome }: Props) {
  const { t } = useI18n();
  return <div className="library-mode-buttons" role="group" aria-label={t("gameModes", "choose")}>
    {(["mods", "install"] as const).map(value => <HoverTooltip text={t("gameModes", value === "install" && modLibrary ? "goHome" : `${value}Hint`)} key={value}>
      <button type="button" disabled={disabled} className={`library-mode-button ${open && !(value === "install" && modLibrary) && (mode === value || (value === "install" && mode === "add")) ? "active" : ""}`}
        aria-pressed={open && !(value === "install" && modLibrary) && (mode === value || (value === "install" && mode === "add"))} onClick={() => value === "install" && modLibrary ? onHome() : onChange(value)}>
        <MenuIcon name={value === "mods" ? "puzzle" : modLibrary ? "home" : "download"} />
        <span>{t("gameModes", value === "install" && modLibrary ? "home" : value)}</span>
      </button>
    </HoverTooltip>)}
  </div>;
}
