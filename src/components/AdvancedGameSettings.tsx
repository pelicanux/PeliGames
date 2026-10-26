import { AdvancedValueSelector } from "./AdvancedValueSelector";
import {invoke,isTauri} from "@tauri-apps/api/core";
import {useGameSaveStatus, GameSaveStatusIndicator} from "./GameSaveStatus";
import { getMangoHudStatus, notifyMangoHudMissing } from "../services/mangohud";
import { WineDllPriorities } from "./WineDllPriorities";
import { LaunchWrappersEditor } from "./LaunchWrappersEditor";
import { TimedFeedback } from "./TimedFeedback";
import { notifyError } from "../services/notifications";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { GamescopeSettingsPanel } from "./GamescopeSettingsPanel";
import { MenuIcon } from "./MenuIcon";
import { useI18n } from "../i18n/I18nContext";
import { advancedOptions, normalizeAdvanced, toggleAdvanced, advancedError, advancedToggleError, advancedOptionHint, advancedOptionValue, defaultGamescope, type AdvancedSettings, gamescopeEnabled } from "../services/advancedSettings";
const groups = [
  { id: "cachyos", title: "Proton-CachyOS Tweaks", icon: "bolt" },
  { id: "hardware", title: "AMD, Mesa e seleção de GPU", icon: "graphics" },
  { id: "nvidia", title: "NVIDIA", icon: "graphics" },
  { id: "intel", title: "Intel e XeSS", icon: "graphics" },
  { id: "input_audio", title: "Controles e áudio", icon: "platform" },
  { id: "diagnostics", title: "Diagnóstico e logs", icon: "logs" },
  { id: "compatibility", title: "Compatibilidade Proton e Wine", icon: "puzzle" },
  { id: "display", title: "Exibição e ferramentas de execução", icon: "platform" },
  { id: "gamescope", title: "Gamescope", icon: "platform" },
  { id: "performance", title: "Desempenho e monitoramento", icon: "chip" },
] as const;
export function AdvancedGameSettings({ value, disabled, reducedMotion, saved, onChange, onBack, launchOptions, hideExecutionEditors = false, embedded = false, automaticDllOverrides, dllOverridesError, onResetDlls, runner }: {
  onResetDlls?: () => void; automaticDllOverrides?: string; dllOverridesError?: string;
  hideExecutionEditors?: boolean; embedded?: boolean; onBack?: () => void; launchOptions?: ReactNode;
  value?: AdvancedSettings; disabled?: boolean; reducedMotion: boolean; saved?: boolean; onChange: (settings: AdvancedSettings) => void;
  isSteam?: boolean; appId?: string; runner?:string;
}) {
  const { t } = useI18n();
  const headerStatus=useGameSaveStatus(saved ? "saved" : "");
  const settings = normalizeAdvanced(value);
  const [capabilities,setCapabilities]=useState<Record<string,string>>({});
  useEffect(()=>{if(!isTauri()||"__PELI_UI_PREVIEW__" in window)return;let active=true;void invoke<Record<string,string>>("get_advanced_capabilities",{runner:runner||null}).then(value=>{if(active)setCapabilities(value);}).catch(()=>{if(active)setCapabilities({});});return()=>{active=false;};},[runner]);
  const reportError = (message: string) => notifyError(t("installedSettings", "advancedTitle"), message);
  const apply = (next: AdvancedSettings) => { const issue = advancedError(next); if (issue) reportError(issue); if (!issue) onChange(next); return !issue; };
  const [checkingMonitoring, setCheckingMonitoring] = useState(false);
  const mounted = useRef(false), checkingRef = useRef(false);
  const latest = useRef({ value, disabled, onChange }); latest.current = { value, disabled, onChange };
  useEffect(() => { mounted.current = true; return () => { mounted.current = false; }; }, []);
  const commitMonitoring = async (next: AdvancedSettings, requiresMonitoring: boolean) => {
    if (disabled || checkingRef.current) return false;
    const issue = advancedError(next); if (issue) reportError(issue);
    if (issue) return false;
    if (!requiresMonitoring) return apply(next);
    checkingRef.current = true; setCheckingMonitoring(true);
    const initialValue = value;
    try {
      const result = await getMangoHudStatus();
      if (!mounted.current || latest.current.disabled || latest.current.value !== initialValue) return false;
      if (result.installed) { latest.current.onChange(next); return true; }
      notifyMangoHudMissing();
      return false;
    } catch { if (mounted.current) reportError("Não foi possível verificar o MangoHUD. Tente novamente."); return false; }
    finally { checkingRef.current = false; if (mounted.current) setCheckingMonitoring(false); }
  };
  const toggleOption = (id: string) => {
    const issue = !settings.options[id]&&capabilities[id] || advancedToggleError(settings, id);
    if (issue) { reportError(issue); return false; }
    const next = toggleAdvanced(settings, id);
    if (["mangohud", "mangohud_env"].includes(id)) next.mangohud_enabled = next.options.mangohud || next.options.mangohud_env;
    return commitMonitoring(next, ["mangohud", "mangohud_env"].includes(id) && next.options[id]);
  };
  const scope = settings.gamescope ?? defaultGamescope;

  const [group, setGroup] = useState<string>();
  const [search, setSearch] = useState("");
  const textMatch = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().includes(search.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase());
  const searching = Boolean(search.trim());
  const foundOptions = advancedOptions.filter(option => option.id !== "steam_client").filter(option => !(hideExecutionEditors && option.id === "dll_overrides")).filter(option => searching ? textMatch(`${option.title} ${option.description} ${option.key} ${groups.find(category => category.id === option.group)?.title}`) : option.group === group);
  const foundLimiter = searching ? textMatch("Limite de FPS MangoHUD individual desempenho monitoramento") : group === "performance";
  const foundLaunch = !hideExecutionEditors && Boolean(launchOptions) && searching && textMatch("Opções de inicialização argumentos variáveis launch command DLL automáticas");
  const foundWrappers = searching && textMatch("Comando do wrapper scripts argumentos empacotador compatibilidade Proton Wine");
  const foundGamescope = searching && textMatch("Gamescope escala upscaling FSR NIS integer inteira esticar imagem resolução largura altura jogo saída janela tela cheia sem borda FPS limitador foco captura cursor opções adicionais");
  return <div className={`installed-advanced-panel${embedded ? "" : " game-info-panel menu-glass-panel"}`}>
    <div className="installed-advanced-heading">{onBack && !group && !searching ? <button type="button" className="installation-name-confirm advanced-return" aria-label={t("installedSettings", "backToSettings")} onClick={onBack}><MenuIcon name="back" /></button> : <MenuIcon name="settings" />}<span>{t("installedSettings", "advancedTitle")}</span><div className="advanced-heading-actions"><div className="advanced-heading-feedback">{checkingMonitoring ? <span role="status">Verificando MangoHUD…</span> : <><GameSaveStatusIndicator/>{saved && !headerStatus && <TimedFeedback className="advanced-saved-feedback">{t("installedSettings", "saved")}</TimedFeedback>}</>}</div><label className="advanced-search"><MenuIcon name="search" /><input type="search" aria-label="Buscar configurações avançadas" placeholder="Busca" value={search} onChange={event => { setSearch(event.target.value); }} onKeyDown={event => { if (event.key === "Escape") setSearch(""); }} /></label></div></div>
    <div className="advanced-pages">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={searching ? "search" : group ?? "categories"} className="advanced-page"
          initial={{ x: reducedMotion ? 0 : 32, opacity: reducedMotion ? 1 : 0 }} animate={{ x: 0, opacity: 1 }}
          exit={{ x: reducedMotion ? 0 : -32, opacity: reducedMotion ? 1 : 0 }} transition={{ duration: reducedMotion ? 0 : .2 }}>
          {!group && !searching ? <div className="advanced-categories">{groups.map(category => <button type="button" key={category.id}
            onClick={() => { setGroup(category.id); }}><MenuIcon name={category.icon} /><span>{category.title}</span><span aria-hidden="true">›</span></button>)}</div> : <>
            <div className="advanced-group-heading"><button type="button" onClick={() => { setSearch(""); setGroup(undefined); }} className="installation-name-confirm"><MenuIcon name="back" />{t("welcome", "back")}</button><span>{searching ? "Resultados da busca" : groups.find(category => category.id === group)?.title}</span></div>
            <fieldset className="advanced-options" disabled={disabled || checkingMonitoring}>
              {!searching && group === "hardware" && <p className="advanced-vendor-label">Opções da GPU e dos drivers</p>}
              {!searching && group === "gamescope" && <GamescopeSettingsPanel value={scope} onChange={gamescope => apply({ ...settings, gamescope, launch_backend: gamescopeEnabled(gamescope) ? "gamescope" : "system" })} />}
              {foundGamescope && <button type="button" className="advanced-search-gamescope" onClick={() => { setSearch(""); setGroup("gamescope"); }}><MenuIcon name="platform" /><span>Gamescope · Escala, FPS, janela e cursor</span><span aria-hidden="true">›</span></button>}
              {searching && !foundOptions.length && !foundLimiter && !foundGamescope && !foundWrappers && !foundLaunch && <p className="advanced-search-empty" role="status">Nenhuma opção encontrada.</p>}
              {foundLimiter && <PerGameFpsLimiter generated={Boolean(settings.afmf?.enabled)} value={settings.mangohud_fps_limit ?? null} onCommit={limit => {
                return commitMonitoring({ ...normalizeAdvanced(latest.current.value), mangohud_fps_limit: limit }, (limit ?? 0) > 0);
              }} />}
              {foundOptions.map(option => option.id === "dll_overrides" ? <WineDllPriorities key={option.id} settings={settings} disabled={disabled} automatic={automaticDllOverrides} detectionError={dllOverridesError} onChange={apply} onReset={onResetDlls} /> : <div className={`advanced-option${option.choices || option.min != null ? " advanced-option-configurable" : ""}`} key={option.id}>
                <div className="advanced-option-description">{searching && <small className="advanced-search-category">{groups.find(category => category.id === option.group)?.title}</small>}<label htmlFor={`advanced-${option.id}`}>{option.title}</label><p>{option.description}</p>{advancedOptionHint(settings, option.id) && <small className="advanced-option-requirement">{advancedOptionHint(settings, option.id)}</small>}<small className="advanced-option-requirement">{capabilities[option.id] ?? (option.key.startsWith("PROTON_")||option.runner_probe ? "O suporte é conferido no runner selecionado ao preparar a execução." : "")}</small><code>{option.kind === "wrapper" ? option.key : `${option.key}=${advancedOptionValue(settings,option.id)}`}</code></div>
                <div className="advanced-option-controls">
                {option.choices && <AdvancedValueSelector label={`Valor: ${option.title}`} value={advancedOptionValue(settings,option.id)} options={option.choices} disabled={disabled || checkingMonitoring} onChange={value => apply({...settings,option_values:{...settings.option_values,[option.id]:value}})} />}
                {option.min!=null&&<input type="number" aria-label={`Valor: ${option.title}`} min={option.min} max={option.max} key={`${option.id}-${advancedOptionValue(settings,option.id)}`} defaultValue={advancedOptionValue(settings,option.id)} onBlur={event=>{const value=event.target.value;if(!apply({...settings,option_values:{...settings.option_values,[option.id]:value}}))event.target.value=advancedOptionValue(settings,option.id);}} onKeyDown={event=>{if(event.key==="Enter")event.currentTarget.blur();}}/>}
                <button id={`advanced-${option.id}`} type="button" role="switch" aria-label={option.title} aria-checked={settings.options[option.id]} className={`advanced-toggle ${settings.options[option.id] ? "enabled" : ""}`} onClick={() => void toggleOption(option.id)}><span /></button>
                </div>
              </div>)}
              {!hideExecutionEditors && ((!searching && group === "compatibility") || foundLaunch) && launchOptions}
              {((!searching && group === "compatibility") || foundWrappers) && <LaunchWrappersEditor value={settings.wrappers ?? []} onChange={wrappers => apply({ ...settings, wrappers })} />}
            </fieldset>
          </>}
        </motion.div>
      </AnimatePresence>
    </div>
  </div>;
}

function PerGameFpsLimiter({ value, onCommit, generated }: { value: number | null; onCommit: (value: number | null) => Promise<boolean>; generated: boolean }) {
  const [draft, setDraft] = useState(value == null ? "" : String(value));
  const [invalid, setInvalid] = useState(false);
  useEffect(() => { setDraft(value == null ? "" : String(value)); }, [value]);
  const commit = () => {
    const next = draft.trim() === "" ? null : Number(draft);
    const bad = next != null && (!Number.isInteger(next) || next < 0 || next > 1000);
    setInvalid(bad);
    if (bad) notifyError("Limite de FPS", "Informe um número inteiro entre 0 e 1000.");
    if (!bad && next !== value) void onCommit(next).then(accepted => { if (!accepted) setDraft(value == null ? "" : String(value)); });
  };
  return <div className="advanced-option advanced-fps-limiter">
    <div className="advanced-option-description"><label htmlFor="game-fps-limit">{generated ? "Limite de FPS total" : "Limite de FPS"}</label>
      <p>Limita apenas este jogo usando MangoHUD, mesmo com o painel oculto. Vazio usa o padrão geral; 0 deixa este jogo sem limite.{generated && " Com AFMF, o limite inclui os quadros gerados."}</p>
    </div>
    <input id="game-fps-limit" type="number" min={0} max={1000} step={1} placeholder="Padrão" aria-label="Limite de FPS" aria-invalid={invalid}
      value={draft} onChange={event => { setDraft(event.target.value); setInvalid(false); }} onBlur={commit}
      onKeyDown={event => { if (event.key === "Enter") { event.preventDefault(); event.currentTarget.blur(); } }} />
  </div>;
}
