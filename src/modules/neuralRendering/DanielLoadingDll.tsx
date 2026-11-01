import { SelectionMenu } from "../../components/SelectionMenu";
import { useI18n } from "../../i18n/I18nContext";
import type { DanielLoadingDllState } from "./danielNr";

export function DanielLoadingDll({ installed, state, value, onChange, disabled }: {
  installed: boolean; state: DanielLoadingDllState; value: string;
  onChange: (value: string) => void; disabled: boolean;
}) {
  const pt = useI18n().language === "pt";
  const options = state.options.map(option => ({
    value: option.name,
    label: option.available ? option.name : `${option.name} · ${pt ? "ocupada" : "occupied"}`,
    disabled: !option.available,
  }));
  if (!installed) options.unshift({ value: "auto", label: pt ? "Automático" : "Automatic", disabled: !state.options.some(option => option.available) });
  return <div className="daniel-loading-dll">
    <div className="modern-dlss-option daniel-loading-dll-row">
      <div><span>{pt ? "DLL de carregamento" : "Loading DLL"}</span>
        <p>{pt ? "Define como o jogo carrega o mod. Use outra opção se o mod não abrir." : "Chooses how the game loads the mod. Try another option if the mod does not open."}</p>
        {installed && <p>{pt ? "A troca é aplicada ao selecionar." : "Changes apply on selection."}</p>}
      </div>
      <div className="daniel-loading-dll-controls">
        <SelectionMenu label={pt ? "DLL de carregamento" : "Loading DLL"} value={value} options={options} onChange={onChange} disabled={disabled || Boolean(state.error)} minMenuWidth={190} />
      </div>
    </div>
    {state.error && <p role="alert" className="daniel-nr-test-error">{state.error}</p>}
  </div>;
}
