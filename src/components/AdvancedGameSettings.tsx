import { getMangoHudStatus, openMonitoringPreferences } from "../services/mangohud";
import { WineDllPriorities } from "./WineDllPriorities";
import { LaunchWrappersEditor } from "./LaunchWrappersEditor";
import { TimedFeedback } from "./TimedFeedback";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { GamescopeSettingsPanel } from "./GamescopeSettingsPanel";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { advancedOptions, normalizeAdvanced, toggleAdvanced, advancedError, defaultGamescope, type AdvancedSettings, gamescopeEnabled } from "../services/advancedSettings";
const groups = [
  { id: "cachyos", title: "Proton-CachyOS Tweaks", icon: "bolt" },
  { id: "hardware", title: "Hardware e drivers", icon: "graphics" },
  { id: "compatibility", title: "Compatibilidade Proton e Wine", icon: "puzzle" },
  { id: "display", title: "Exibição e ferramentas de execução", icon: "platform" },
  { id: "gamescope", title: "Gamescope", icon: "platform" },
  { id: "performance", title: "Desempenho e monitoramento", icon: "chip" },
] as const;
export function AdvancedGameSettings({ value, disabled, reducedMotion, saved, onChange, onBack, launchOptions, hideExecutionEditors = false, embedded = false, automaticDllOverrides, dllOverridesError, onResetDlls }: {
  onResetDlls?: () => void; automaticDllOverrides?: string; dllOverridesError?: string;
  hideExecutionEditors?: boolean; embedded?: boolean; onBack?: () => void; launchOptions?: ReactNode;
  value?: AdvancedSettings; disabled?: boolean; reducedMotion: boolean; saved?: boolean; onChange: (settings: AdvancedSettings) => void;
}) {
  const { t } = useI18n();
  const settings = normalizeAdvanced(value);
  const [error, setError] = useState("");
  const apply = (next: AdvancedSettings) => { const issue = advancedError(next); setError(issue ?? ""); if (!issue) onChange(next); return !issue; };
  const [monitoringNotice, setMonitoringNotice] = useState(false);
  const [checkingMonitoring, setCheckingMonitoring] = useState(false);
  const mounted = useRef(false), checkingRef = useRef(false);
  const latest = useRef({ value, disabled, onChange }); latest.current = { value, disabled, onChange };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const commitMonitoring = async (next: AdvancedSettings, requiresMonitoring: boolean) => {
    if (disabled || checkingRef.current) return false;
    const issue = advancedError(next); setError(issue ?? "");
    if (issue) return false;
    if (!requiresMonitoring) return apply(next);
    checkingRef.current = true; setCheckingMonitoring(true); setMonitoringNotice(false);
    const initialValue = value;
    try {
      const result = await getMangoHudStatus();
      if (!mounted.current || latest.current.disabled || latest.current.value !== initialValue) return false;
      if (result.installed) { latest.current.onChange(next); return true; }
      setMonitoringNotice(true);
      return false;
    } catch { if (mounted.current) setError("Não foi possível verificar o MangoHUD. Tente novamente."); return false; }
    finally { checkingRef.current = false; if (mounted.current) setCheckingMonitoring(false); }
  };
  const toggleOption = (id: string) => {
    const next = toggleAdvanced(settings, id);
    if (["mangohud", "mangohud_env"].includes(id)) next.mangohud_enabled = next.options.mangohud || next.options.mangohud_env;
    return commitMonitoring(next, ["mangohud", "mangohud_env"].includes(id) && next.options[id]);
  };
  const scope = settings.gamescope ?? defaultGamescope;

  const [group, setGroup] = useState<string>();
  const [search, setSearch] = useState("");
  const textMatch = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().includes(search.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase());
  const searching = Boolean(search.trim());
  const foundOptions = advancedOptions.filter(option => !(hideExecutionEditors && option.id === "dll_overrides")).filter(option => searching ? textMatch(`${option.title} ${option.description} ${option.key} ${groups.find(category => category.id === option.group)?.title}`) : option.group === group);
  const foundLimiter = searching ? textMatch("Limite de FPS MangoHUD individual desempenho monitoramento") : group === "performance";
  const foundLaunch = !hideExecutionEditors && Boolean(launchOptions) && searching && textMatch("Opções de inicialização argumentos variáveis launch command DLL automáticas");
  const foundWrappers = searching && textMatch("Comando do wrapper scripts argumentos empacotador compatibilidade Proton Wine");
  const foundGamescope = searching && textMatch("Gamescope escala upscaling FSR NIS integer inteira esticar imagem resolução largura altura jogo saída janela tela cheia sem borda FPS limitador foco captura cursor opções adicionais");
  return <div className={`installed-advanced-panel${embedded ? "" : " game-info-panel menu-glass-panel"}`}>
    <div className="installed-advanced-heading">{onBack && !group && !searching ? <button type="button" className="installation-name-confirm advanced-return" aria-label={t("installedSettings", "backToSettings")} onClick={onBack}><MenuIcon name="back" /></button> : <MenuIcon name="settings" />}<span>{t("installedSettings", "advancedTitle")}</span><label className="advanced-search"><MenuIcon name="search" /><input type="search" aria-label="Buscar configurações avançadas" placeholder="Busca" value={search} onChange={event => { setSearch(event.target.value); setError(""); }} onKeyDown={event => { if (event.key === "Escape") setSearch(""); }} /></label></div>
    {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}
    {checkingMonitoring && <p role="status" className="advanced-monitoring-notice">Verificando MangoHUD…</p>}
    {monitoringNotice && <div role="alert" className="advanced-monitoring-notice">MangoHUD não foi detectado. <button type="button" onClick={openMonitoringPreferences}>Clique aqui para instalar</button></div>}
    <div className="advanced-pages">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={searching ? "search" : group ?? "categories"} className="advanced-page"
          initial={{ x: reducedMotion ? 0 : 32, opacity: reducedMotion ? 1 : 0 }} animate={{ x: 0, opacity: 1 }}
          exit={{ x: reducedMotion ? 0 : -32, opacity: reducedMotion ? 1 : 0 }} transition={{ duration: reducedMotion ? 0 : .2 }}>
          {!group && !searching ? <div className="advanced-categories">{groups.map(category => <button type="button" key={category.id}
            onClick={() => { setError(""); setGroup(category.id); }}><MenuIcon name={category.icon} /><span>{category.title}</span><span aria-hidden="true">›</span></button>)}</div> : <>
            <div className="advanced-group-heading"><button type="button" onClick={() => { setError(""); setSearch(""); setGroup(undefined); }} className="installation-name-confirm"><MenuIcon name="back" />{t("welcome", "back")}</button><span>{searching ? "Resultados da busca" : groups.find(category => category.id === group)?.title}</span></div>
            <fieldset className="advanced-options" disabled={disabled || checkingMonitoring}>
              {!searching && group === "hardware" && <p className="advanced-vendor-label">Opções da GPU · AMD</p>}
              {!searching && group === "gamescope" && <GamescopeSettingsPanel value={scope} onChange={gamescope => apply({ ...settings, gamescope, launch_backend: gamescopeEnabled(gamescope) ? "gamescope" : "system" })} />}
              {foundGamescope && <button type="button" className="advanced-search-gamescope" onClick={() => { setSearch(""); setGroup("gamescope"); }}><MenuIcon name="platform" /><span>Gamescope · Escala, FPS, janela e cursor</span><span aria-hidden="true">›</span></button>}
              {searching && !foundOptions.length && !foundLimiter && !foundGamescope && !foundWrappers && !foundLaunch && <p className="advanced-search-empty" role="status">Nenhuma opção encontrada.</p>}
              {foundLimiter && <PerGameFpsLimiter value={settings.mangohud_fps_limit ?? null} onCommit={limit => {
                return commitMonitoring({ ...normalizeAdvanced(latest.current.value), mangohud_fps_limit: limit }, (limit ?? 0) > 0);
              }} />}
              {foundOptions.map(option => option.id === "dll_overrides" ? <WineDllPriorities key={option.id} settings={settings} disabled={disabled} automatic={automaticDllOverrides} detectionError={dllOverridesError} onChange={apply} onReset={onResetDlls} /> : <div className="advanced-option" key={option.id}>
                <div className="advanced-option-description">{searching && <small className="advanced-search-category">{groups.find(category => category.id === option.group)?.title}</small>}<label htmlFor={`advanced-${option.id}`}>{option.title}</label><p>{option.description}</p><code>{option.kind === "wrapper" ? option.key : `${option.key}=${option.value}`}</code></div>
                <button id={`advanced-${option.id}`} type="button" role="switch" aria-label={option.title} aria-checked={settings.options[option.id]} disabled={option.id === "hdr" && !settings.options.wayland} className={`advanced-toggle ${settings.options[option.id] ? "enabled" : ""}`} onClick={() => void toggleOption(option.id)}><span /></button>
              </div>)}
              {!hideExecutionEditors && ((!searching && group === "compatibility") || foundLaunch) && launchOptions}
              {((!searching && group === "compatibility") || foundWrappers) && <LaunchWrappersEditor value={settings.wrappers ?? []} onChange={wrappers => apply({ ...settings, wrappers })} />}
            </fieldset>
          </>}
        </motion.div>
      </AnimatePresence>
    </div>
    {saved && <TimedFeedback>{t("installedSettings", "saved")}</TimedFeedback>}
  </div>;
}

function PerGameFpsLimiter({ value, onCommit }: { value: number | null; onCommit: (value: number | null) => Promise<boolean> }) {
  const [draft, setDraft] = useState(value == null ? "" : String(value));
  const [invalid, setInvalid] = useState(false);
  useEffect(() => { setDraft(value == null ? "" : String(value)); }, [value]);
  const commit = () => {
    const next = draft.trim() === "" ? null : Number(draft);
    const bad = next != null && (!Number.isInteger(next) || next < 0 || next > 1000);
    setInvalid(bad);
    if (!bad && next !== value) void onCommit(next).then(accepted => { if (!accepted) setDraft(value == null ? "" : String(value)); });
  };
  return <div className="advanced-option advanced-fps-limiter">
    <div className="advanced-option-description"><label htmlFor="game-fps-limit">Limite de FPS</label>
      <p>Limita apenas este jogo usando MangoHUD, mesmo com o painel oculto. Vazio usa o padrão geral; 0 deixa este jogo sem limite.</p>
      {invalid && <p role="alert">Informe um número inteiro entre 0 e 1000.</p>}
    </div>
    <input id="game-fps-limit" type="number" min={0} max={1000} step={1} placeholder="Padrão" aria-label="Limite de FPS" aria-invalid={invalid}
      value={draft} onChange={event => { setDraft(event.target.value); setInvalid(false); }} onBlur={commit}
      onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); event.currentTarget.blur(); } }} />
  </div>;
}
