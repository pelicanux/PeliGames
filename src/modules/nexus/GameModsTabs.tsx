import {useId, type ReactNode} from "react";
import {useI18n} from "../../i18n/I18nContext";
import "./gameModsTabs.css";

type ModsTab = "nexus" | "dlss";
const tabs: {id: ModsTab; label: string}[] = [
  {id: "nexus", label: "Nexus Mods"},
  {id: "dlss", label: "DLSSNR-AMD"},
];

export function GameModsTabs({activeTab, onTabChange, nexus, dlss}: {
  activeTab: ModsTab;
  onTabChange: (tab: ModsTab) => void;
  nexus: ReactNode;
  dlss: ReactNode;
}) {
  const id = useId();
  const pt = useI18n().language === "pt";
  return <div className="game-modules">
    <div className="settings-tab-toolbar">
      <div className="preference-mod-tabs game-modules-tabs" role="tablist"
        aria-label={pt ? "Mods deste jogo" : "Mods for this game"} data-active={activeTab}
        onKeyDown={event => {
          if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) return;
          event.preventDefault();
          const next = event.key === "Home" ? "nexus" : event.key === "End" ? "dlss" : activeTab === "nexus" ? "dlss" : "nexus";
          onTabChange(next);
          document.getElementById(`${id}-tab-${next}`)?.focus();
        }}>
        {tabs.map(tab => <button key={tab.id} type="button" role="tab"
          id={`${id}-tab-${tab.id}`} aria-selected={activeTab === tab.id}
          aria-controls={`${id}-panel-${tab.id}`} tabIndex={activeTab === tab.id ? 0 : -1}
          onClick={() => onTabChange(tab.id)}>{tab.label}</button>)}
      </div>
    </div>
    <div role="tabpanel" id={`${id}-panel-${activeTab}`} aria-labelledby={`${id}-tab-${activeTab}`}>
      {activeTab === "nexus" ? nexus : dlss}
    </div>
  </div>;
}
