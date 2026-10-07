import { LaunchWrappersEditor } from "./LaunchWrappersEditor";
import { TimedFeedback } from "./TimedFeedback";
import { useEffect, useState } from "react";
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
export function AdvancedGameSettings({ value, disabled, reducedMotion, saved, onChange }: {
  value?: AdvancedSettings; disabled?: boolean; reducedMotion: boolean; saved?: boolean; onChange: (settings: AdvancedSettings) => void;
}) {
  const { t } = useI18n();
  const settings = normalizeAdvanced(value);
  const [error, setError] = useState("");
  const apply = (next: AdvancedSettings) => { const issue = advancedError(next); setError(issue ?? ""); if (!issue) onChange(next); return !issue; };
  const scope = settings.gamescope ?? defaultGamescope;

  const [group, setGroup] = useState<string>();
  const [search, setSearch] = useState("");
  const textMatch = (text: string) => text.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().includes(search.trim().normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase());
  const searching = Boolean(search.trim());
  const foundOptions = advancedOptions.filter(option => searching ? textMatch(`${option.title} ${option.description} ${option.key} ${groups.find(category => category.id === option.group)?.title}`) : option.group === group);
  const foundWrappers = searching && textMatch("Comando do wrapper scripts argumentos empacotador compatibilidade Proton Wine");
  const foundGamescope = searching && textMatch("Gamescope escala upscaling FSR NIS integer inteira esticar imagem resolução largura altura jogo saída janela tela cheia sem borda FPS limitador foco captura cursor opções adicionais");
  const [dll, setDll] = useState(settings.dll_overrides);
  useEffect(() => { setDll(value?.dll_overrides ?? "dxgi=n,b"); }, [value?.dll_overrides]);
  return <div className="game-info-panel menu-glass-panel installed-advanced-panel">
    <div className="installed-advanced-heading"><MenuIcon name="settings" /><span>{t("installedSettings", "advancedTitle")}</span><label className="advanced-search"><MenuIcon name="search" /><input type="search" aria-label="Buscar configurações avançadas" placeholder="Busca" value={search} onChange={event => { setSearch(event.target.value); setError(""); }} onKeyDown={event => { if (event.key === "Escape") setSearch(""); }} /></label></div>
    {error && <TimedFeedback role="alert" className="advanced-validation-error">{error}</TimedFeedback>}
    <div className="advanced-pages">
      <AnimatePresence mode="wait" initial={false}>
        <motion.div key={searching ? "search" : group ?? "categories"} className="advanced-page"
          initial={{ x: reducedMotion ? 0 : 32, opacity: reducedMotion ? 1 : 0 }} animate={{ x: 0, opacity: 1 }}
          exit={{ x: reducedMotion ? 0 : -32, opacity: reducedMotion ? 1 : 0 }} transition={{ duration: reducedMotion ? 0 : .2 }}>
          {!group && !searching ? <div className="advanced-categories">{groups.map(category => <button type="button" key={category.id}
            onClick={() => { setError(""); setGroup(category.id); }}><MenuIcon name={category.icon} /><span>{category.title}</span><span aria-hidden="true">›</span></button>)}</div> : <>
            <div className="advanced-group-heading"><button type="button" onClick={() => { setError(""); setSearch(""); setGroup(undefined); }} className="installation-name-confirm"><MenuIcon name="back" />{t("welcome", "back")}</button><span>{searching ? "Resultados da busca" : groups.find(category => category.id === group)?.title}</span></div>
            <fieldset className="advanced-options" disabled={disabled}>
              {!searching && group === "hardware" && <p className="advanced-vendor-label">Opções da GPU · AMD</p>}
              {!searching && group === "gamescope" && <GamescopeSettingsPanel value={scope} onChange={gamescope => apply({ ...settings, gamescope, launch_backend: gamescopeEnabled(gamescope) ? "gamescope" : "system" })} />}
              {foundGamescope && <button type="button" className="advanced-search-gamescope" onClick={() => { setSearch(""); setGroup("gamescope"); }}><MenuIcon name="platform" /><span>Gamescope · Escala, FPS, janela e cursor</span><span aria-hidden="true">›</span></button>}
              {searching && !foundOptions.length && !foundGamescope && !foundWrappers && <p className="advanced-search-empty" role="status">Nenhuma opção encontrada.</p>}
              {foundOptions.map(option => <div className="advanced-option" key={option.id}>
                <div className="advanced-option-description">{searching && <small className="advanced-search-category">{groups.find(category => category.id === option.group)?.title}</small>}<label htmlFor={`advanced-${option.id}`}>{option.title}</label><p>{option.description}</p><code>{option.kind === "wrapper" ? option.key : `${option.key}=${option.id === "dll_overrides" ? settings.dll_overrides : option.value}`}</code></div>
                <button id={`advanced-${option.id}`} type="button" role="switch" aria-label={option.title} aria-checked={settings.options[option.id]} disabled={option.id === "hdr" && !settings.options.wayland} className={`advanced-toggle ${settings.options[option.id] ? "enabled" : ""}`} onClick={() => apply(toggleAdvanced(settings, option.id))}><span /></button>
                {option.id === "dll_overrides" && settings.options.dll_overrides && <form className="advanced-dll-editor" onSubmit={event => { event.preventDefault(); apply({ ...settings, dll_overrides: dll.trim() }); }}>
                  <input aria-label="DLLs do Wine" value={dll} onChange={event => setDll(event.target.value)} placeholder="dxgi=n,b" />
                  {dll !== settings.dll_overrides && <button type="submit" className="installation-name-confirm"><MenuIcon name="check" />{t("gameModes", "confirmName")}</button>}
                </form>}
              </div>)}
              {((!searching && group === "compatibility") || foundWrappers) && <LaunchWrappersEditor value={settings.wrappers ?? []} onChange={wrappers => apply({ ...settings, wrappers })} />}
            </fieldset>
          </>}
        </motion.div>
      </AnimatePresence>
    </div>
    {saved && <TimedFeedback>{t("installedSettings", "saved")}</TimedFeedback>}
  </div>;
}
