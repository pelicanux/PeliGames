import { SelectionMenu } from "../../components/SelectionMenu";
import { useI18n } from "../../i18n/I18nContext";
import { openDanielPreferences, type DanielState } from "./danielNr";

export function DanielRuntimeVersion({ state, value, onChange, onRecover, disabled }: {
  state: DanielState; value: string; onChange: (value: string) => void;
  onRecover: () => void; disabled: boolean;
}) {
  const {t: tx} = useI18n();

  const missing = state.runtimes.some(r => !r.ready);
  return <div className="daniel-loading-dll">
    <div className="modern-dlss-option daniel-loading-dll-row"><div>
      <span>{tx("ui-daniel-runtime-version", "runtime_version")}</span>
      <p>{state.installed
        ? (tx("ui-daniel-runtime-version", "selecting_automatically_switches_the_version_each_version_s_settings_are_preserved"))
        : (tx("ui-daniel-runtime-version", "choose_the_version_to_install_for_this_game"))}</p>
    </div><div className="daniel-loading-dll-controls">
      <SelectionMenu label={tx("ui-daniel-runtime-version", "daniel_runtime_version")} value={value}
        options={state.runtimes.map(r => ({ value: r.version, label: r.version + (!r.ready ? (tx("ui-daniel-runtime-version", "prepare")) : ""), disabled: state.installed && !r.ready }))}
        onChange={onChange} disabled={disabled || state.pending_version_change} minMenuWidth={190}/>
    </div></div>
    {value === "0.8.0" && <p className="modern-dlss-description daniel-nr-note">{tx("ui-daniel-runtime-version", "0_8_0_experimental_if_problems_occur_select_0_6_0_to_return")}</p>}
    {missing && state.installed && <button type="button" className="modern-mini-btn" disabled={disabled} onClick={openDanielPreferences}>{tx("ui-daniel-runtime-version", "prepare_other_versions")}</button>}
    {state.pending_version_change && <><p role="alert" className="daniel-nr-test-error">{tx("ui-daniel-runtime-version", "a_version_change_was_interrupted_recover_the_previous_version_to_continue")}</p><button type="button" className="modern-mini-btn" disabled={disabled} onClick={onRecover}>{tx("ui-daniel-runtime-version", "recover_previous_version")}</button></>}
  </div>;
}
