import { SelectionMenu } from "../../components/SelectionMenu";
import { useI18n } from "../../i18n/I18nContext";
import type { DanielLoadingDllState } from "./danielNr";

export function DanielLoadingDll({ installed, state, value, onChange, disabled }: {
  installed: boolean; state: DanielLoadingDllState; value: string;
  onChange: (value: string) => void; disabled: boolean;
}) {
  const {t: tx} = useI18n();

  const options = state.options.map(option => ({
    value: option.name,
    label: option.available ? option.name : `${option.name} · ${tx("ui-daniel-loading-dll", "occupied")}`,
    disabled: !option.available,
  }));
  if (!installed) options.unshift({ value: "auto", label: tx("ui-daniel-loading-dll", "automatic"), disabled: !state.options.some(option => option.available) });
  return <div className="daniel-loading-dll">
    <div className="modern-dlss-option daniel-loading-dll-row">
      <div><span>{tx("ui-daniel-loading-dll", "loading_dll")}</span>
        <p>{tx("ui-daniel-loading-dll", "chooses_how_the_game_loads_the_mod_try_another_option_if_the_mod_does_not_open")}</p>
        {installed && <p>{tx("ui-daniel-loading-dll", "changes_apply_on_selection")}</p>}
      </div>
      <div className="daniel-loading-dll-controls">
        <SelectionMenu label={tx("ui-daniel-loading-dll", "loading_dll")} value={value} options={options} onChange={onChange} disabled={disabled || Boolean(state.error)} minMenuWidth={190} />
      </div>
    </div>
    {state.error && <p role="alert" className="daniel-nr-test-error">{state.error}</p>}
  </div>;
}
