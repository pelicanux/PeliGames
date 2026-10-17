import { OPEN_DISCORD_PANEL_EVENT } from "../services/communityLinks";
import React, { useEffect, useState } from 'react';
import { DiscordPanel } from './DiscordPanel';
import { MatrixChatPanel } from './MatrixChatPanel';
import { MatrixStreamDock } from './MatrixStreamDock';
import { OPEN_MATRIX_CHAT_EVENT, startMatrixChat } from '../services/matrixChat';
import { BrandName } from './BrandName';
import { RunningGamesIndicator, type RunningGameSummary } from './RunningGamesIndicator';
import { useI18n } from '../i18n/I18nContext';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { isTauri } from '@tauri-apps/api/core';


interface TitleBarProps {
  onShowCredits: () => void;
  onHome: () => void;
  homeDisabled?: boolean;
  settingsMenu?: React.ReactNode;
  updateIndicator?: React.ReactNode;
  notificationIndicator?: React.ReactNode;
  disabled?: boolean;
  runningGames?: RunningGameSummary[];
}

export const TitleBar: React.FC<TitleBarProps> = ({ onShowCredits, onHome, homeDisabled, settingsMenu, updateIndicator, notificationIndicator, disabled, runningGames = [] }) => {
  const { t } = useI18n();
  const appWindow = isTauri() ? getCurrentWindow() : null;

  const [showDiscord, setShowDiscord] = useState(false);
  const [showMatrix, setShowMatrix] = useState(false);
  // An existing Matrix sign-in resumes in the background (mentions reach the bell).
  useEffect(() => {
    startMatrixChat();
    const open = () => setShowMatrix(true);
    const openDiscord = () => setShowDiscord(true);
    window.addEventListener(OPEN_MATRIX_CHAT_EVENT, open);
    window.addEventListener(OPEN_DISCORD_PANEL_EVENT, openDiscord);
    return () => { window.removeEventListener(OPEN_MATRIX_CHAT_EVENT, open); window.removeEventListener(OPEN_DISCORD_PANEL_EVENT, openDiscord); };
  }, []);

  return (
    <>
    {showDiscord && <DiscordPanel onClose={() => setShowDiscord(false)} />}
    {showMatrix && <MatrixChatPanel onClose={() => setShowMatrix(false)} />}
    <MatrixStreamDock />
    {/* With a dialog over the launcher, the window can still be dragged, and the
        notifications and running games stay usable; the window controls, home,
        credits, settings and update are off so a dialog is not closed by mistake. */}
    <div className="app-titlebar"
      data-tauri-drag-region="true"
      style={{
        height: '50px',
        background: 'rgba(var(--surface-26-26-26, 26, 26, 26), 0.4)',
        backdropFilter: 'blur(24px) saturate(1.2)',
        WebkitBackdropFilter: 'blur(24px) saturate(1.2)',
        display: 'flex',
        justifyContent: 'space-between',
        alignItems: 'center',
        padding: '0 16px',
        borderBottom: '1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)',
        userSelect: 'none',
        borderTopLeftRadius: '12px',
        borderTopRightRadius: '12px',
        position: 'relative',
        zIndex: 2000
      }}
    >
      {/* Left section: Icon and Title */}
      <div data-tauri-drag-region="true" style={{ display: 'flex', alignItems: 'center', gap: '12.8px', height: '100%' }}>
        <img src="/peligames.svg" alt="Icon" style={{ width: '24px', height: '24px', objectFit: 'contain', pointerEvents: 'none' }} />
        <button type="button" className="titlebar-title titlebar-home" data-tauri-drag-region="false" onClick={onHome} disabled={disabled || homeDisabled} aria-label={t("gameModes", "goHome")} title={t("gameModes", "goHome")} style={{ 
          margin: 0, 
          color: "var(--tone-edf4ff, #edf4ff)" 
        }}>
          <BrandName />
        </button>
        <RunningGamesIndicator games={runningGames} />
      </div>

      {/* Right section: Links and Window Controls */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '8px', height: '100%' }}>
        <div className="titlebar-group" style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
        
        {updateIndicator}
        <div className="titlebar-optional-actions" inert={disabled} style={{ display: "flex", alignItems: "center", opacity: disabled ? 0.3 : 1, pointerEvents: disabled ? "none" : "auto" }}>
          <button 
            data-tauri-drag-region="false"
            className="btn-titlebar"
            disabled={disabled}
            onClick={onShowCredits}
            title={t("credits", "button")}
            style={{ background: 'transparent', border: 'none', color: 'var(--tone-94a3b8, #94a3b8)', cursor: 'pointer', padding: '0.3rem 0.5rem', display: 'flex', alignItems: 'center', borderRadius: '4px' }}
          >
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
          </button>
          {settingsMenu && (
            <div data-tauri-drag-region="false" style={{ display: 'flex', alignItems: 'center' }}>
              {settingsMenu}
            </div>
          )}
        </div>
        {notificationIndicator}
        </div>

        <div className="titlebar-separator" style={{ width: '1px', height: '20px', background: 'rgba(var(--surface-255-255-255, 255, 255, 255), 0.1)', marginRight: '8px' }}></div>

        <button 
          data-tauri-drag-region="false"
          className="btn-titlebar window-control"
          disabled={disabled || !appWindow}
          onClick={() => appWindow?.minimize()}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="5" y1="12" x2="19" y2="12"></line></svg>
        </button>
        <button 
          data-tauri-drag-region="false"
          className="btn-titlebar window-control"
          disabled={disabled || !appWindow}
          onClick={() => appWindow?.toggleMaximize()}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect></svg>
        </button>
        <button 
          data-tauri-drag-region="false"
          className="btn-titlebar window-control close"
          disabled={disabled || !appWindow}
          onClick={() => appWindow?.close()}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><line x1="18" y1="6" x2="6" y2="18"></line><line x1="6" y1="6" x2="18" y2="18"></line></svg>
        </button>
      </div>
    </div>
    </>
  );
};
