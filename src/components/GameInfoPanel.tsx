import type { ReactNode } from "react";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";

interface Props {
  directoryRow: ReactNode;
  release: string;
  platform: string;
  children?: ReactNode;
}

export function GameInfoPanel({ directoryRow, release, platform, children }: Props) {
  const { t } = useI18n();
  return <div className="game-info-panel menu-glass-panel">
    <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", marginBottom: "0.5rem" }}>
      <MenuIcon name="document" />
      <span style={{ color: "var(--tone-ffffff, #fff)", fontWeight: "bold", fontSize: "0.95rem" }}>{t("gameInfo", "title")}</span>
    </div>
    <div className="game-info-fields">
      {directoryRow}
      <div className="game-info-item">
        <span className="game-info-label"><MenuIcon name="calendar" />{t("gameInfo", "release")}</span>
        <span className="info-value-pill info-release">{release}</span>
      </div>
      <div className="game-info-item">
        <span className="game-info-label"><MenuIcon name="platform" />{t("gameInfo", "platform")}</span>
        <span className="info-value-pill info-platform">{platform}</span>
      </div>
      {children}
    </div>
  </div>;
}
