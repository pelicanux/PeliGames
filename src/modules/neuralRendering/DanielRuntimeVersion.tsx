import { SelectionMenu } from "../../components/SelectionMenu";
import { useI18n } from "../../i18n/I18nContext";
import { openDanielPreferences, type DanielState } from "./danielNr";

export function DanielRuntimeVersion({ state, value, onChange, onRecover, disabled }: {
  state: DanielState; value: string; onChange: (value: string) => void;
  onRecover: () => void; disabled: boolean;
}) {
  const pt = useI18n().language === "pt";
  const missing = state.runtimes.some(r => !r.ready);
  return <div className="daniel-loading-dll">
    <div className="modern-dlss-option daniel-loading-dll-row"><div>
      <span>{pt ? "Versão do motor" : "Runtime version"}</span>
      <p>{state.installed
        ? (pt ? "Ao selecionar, a versão é trocada automaticamente. Os ajustes de cada versão são preservados." : "Selecting automatically switches the version. Each version’s settings are preserved.")
        : (pt ? "Escolha a versão que será instalada neste jogo." : "Choose the version to install for this game.")}</p>
    </div><div className="daniel-loading-dll-controls">
      <SelectionMenu label={pt ? "Versão do motor Daniel" : "Daniel runtime version"} value={value}
        options={state.runtimes.map(r => ({ value: r.version, label: r.version + (!r.ready ? (pt ? " · Preparar" : " · Prepare") : ""), disabled: state.installed && !r.ready }))}
        onChange={onChange} disabled={disabled || state.pending_version_change} minMenuWidth={190}/>
    </div></div>
    {value === "0.8.0" && <p className="modern-dlss-description daniel-nr-note">{pt ? "0.8.0 experimental. Se houver problemas, selecione 0.6.0 para voltar." : "0.8.0 experimental. If problems occur, select 0.6.0 to return."}</p>}
    {missing && state.installed && <button type="button" className="modern-mini-btn" disabled={disabled} onClick={openDanielPreferences}>{pt ? "Preparar outras versões" : "Prepare other versions"}</button>}
    {state.pending_version_change && <><p role="alert" className="daniel-nr-test-error">{pt ? "Uma troca de versão foi interrompida. Recupere a versão anterior para continuar." : "A version change was interrupted. Recover the previous version to continue."}</p><button type="button" className="modern-mini-btn" disabled={disabled} onClick={onRecover}>{pt ? "Recuperar versão anterior" : "Recover previous version"}</button></>}
  </div>;
}
