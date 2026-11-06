import {useId, type ReactNode} from "react";
import {useI18n} from "../../i18n/I18nContext";
import {experimentalDanielNr} from "../../services/releaseFeatures";
import "./gameModsTabs.css";

export type GameModsTab = "nexus" | "dlss" | "daniel";
const allTabs: {id: GameModsTab; label: string}[] = [
  {id: "nexus", label: "Nexus Mods"},
  {id: "dlss", label: "DLSSNR-AMD"},
  {id: "daniel", label: "Daniel NR"},
];
const tabs = allTabs.filter(tab => tab.id !== "daniel" || experimentalDanielNr);

export function GameModsTabs({activeTab, onTabChange, nexus, dlss, daniel}: {
  activeTab: GameModsTab;
  onTabChange: (tab: GameModsTab) => void;
  nexus: ReactNode;
  dlss: ReactNode;
  daniel: ReactNode;
}) {
  const {t: tx} = useI18n();

  const id = useId();
  const visibleTab = tabs.some(tab => tab.id === activeTab) ? activeTab : "nexus";

  const panels: Record<GameModsTab, ReactNode> = {nexus, dlss, daniel};
  return <div className="game-modules">
    <div className="settings-tab-toolbar">
      <div className="preference-mod-tabs game-modules-tabs" role="tablist"
        aria-label={tx("ui-game-mods-tabs", "mods_for_this_game")} data-active={visibleTab} data-count={tabs.length}
        onKeyDown={event => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const index = tabs.findIndex(tab => tab.id === visibleTab);
          const nextIndex = event.key === "Home" ? 0 : event.key === "End" ? tabs.length - 1
            : (index + (event.key === "ArrowRight" ? 1 : -1) + tabs.length) % tabs.length;
          const next = tabs[nextIndex].id;
          onTabChange(next);
          document.getElementById(`${id}-tab-${next}`)?.focus();
        }}>
        {tabs.map(tab => <button key={tab.id} type="button" role="tab"
          id={`${id}-tab-${tab.id}`} aria-selected={visibleTab === tab.id}
          aria-controls={`${id}-panel-${tab.id}`} tabIndex={visibleTab === tab.id ? 0 : -1}
          onClick={() => onTabChange(tab.id)}>{tab.id === "daniel" ? (tx("ui-game-mods-tabs", "daniel_nr_test")) : tab.label}</button>)}
      </div>
    </div>
    <div role="tabpanel" id={`${id}-panel-${visibleTab}`} aria-labelledby={`${id}-tab-${visibleTab}`}>
      {panels[visibleTab]}
    </div>
  </div>;
}
