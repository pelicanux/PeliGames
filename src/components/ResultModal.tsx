import { LogText } from "./LogText";
import { ModalSurface } from "./ModalSurface";
import { createPortal } from "react-dom";
import { type ReactNode, useState } from "react";
import { useI18n } from "../i18n/I18nContext";
import "../App.css";

interface Props {
  title: string;
  message: string;
  type: "success" | "error";
  logs: string;
  onClose: () => void;
  onOpenSetup?: () => void;
  showLogButton?: boolean;
  children?: ReactNode;
}

export const ResultModal: React.FC<Props> = ({ title, message, type, logs, onClose, onOpenSetup, showLogButton = true, children }) => {
  const [showLogs, setShowLogs] = useState(false);
  const { t } = useI18n();

  return createPortal(
    <div className="modal-overlay">
      <ModalSurface header={<h2>{title}</h2>} onDismiss={onClose} className={`modal-content result-modal ${type}`} style={{ maxWidth: showLogs ? '800px' : '680px' }}>
        
        <div className="modal-body">
          {message.split("\n").map((line, idx) => {
            const isWarning = line.includes(t("installation", "incompatibleTitle")) || line.includes(t("installation", "incompatibleBody"));
            return (
              <p 
                key={idx}
                style={isWarning ? {
                  color: "var(--tone-f87171, #f87171)",
                  fontWeight: 600,
                  background: "rgba(239, 68, 68, 0.1)",
                  padding: "0.8rem",
                  borderRadius: "8px",
                  border: "1px solid rgba(239, 68, 68, 0.3)",
                  marginTop: "0.5rem",
                  lineHeight: "1.5"
                } : { lineHeight: "1.5" }}
              >
                {line}
              </p>
            );
          })}
          
          {children}
          {showLogs && (
            <div className="console game-log-output" style={{ marginTop: '1.5rem', maxHeight: '300px', overflowY: 'auto', whiteSpace: 'pre-wrap', wordBreak: 'break-word', textAlign: 'left' }}>
              <LogText content={logs}/>
            </div>
          )}
        </div>
        <div className="modal-actions" style={{ flexWrap: "wrap" }}>
          {onOpenSetup && <button className="btn btn-primary" onClick={onOpenSetup}>{t("installation", "openSetup")}</button>}
          {showLogButton && <button className="btn-secondary" onClick={() => setShowLogs(!showLogs)}>
            {showLogs ? t("resultModal", "hideLog") : t("resultModal", "viewLog")}
          </button>}
          <button className="btn-primary" onClick={onClose}>{t("resultModal", "close")}</button>
        </div>
      </ModalSurface>
    </div>,
    document.body
  );
};
