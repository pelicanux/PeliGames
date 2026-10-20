import type { ReactNode } from "react";
import { useI18n } from "../../i18n/I18nContext";
import { HoverTooltip } from "../HoverTooltip";
import { ModOptionSwitch } from "../ModOptionRow";
import type { InstallRoute } from "../../types/installer";

interface Props {
  route: InstallRoute;
  onRoute: (route: InstallRoute) => void;
  bitness: "32" | "64";
  onBitness: (bitness: "32" | "64") => void;
  gpuArch: "rdna3" | "rdna4";
  recommendation?: string;
  recommendedBitness?: "32" | "64" | null;
  recommendOptiscaler?: boolean;
  busy: boolean;
  startup: boolean;
  startupDisabled: boolean;
  onStartup: () => void;
  renodx: boolean;
  onRenodx: () => void;
  model: "small" | "original";
  modelAvailable: boolean;
  unifiedNetworks: boolean;
  lumeniteAvailable: boolean;
  lumenite: boolean;
  onLumenite: (enabled: boolean) => void;
  installed?: boolean;
  modelStatus: "checking" | "error" | "backend" | "missing" | "ready";
  onModel: (model: "small" | "original") => void;
  int4: boolean;
  onInt4: () => void;
  actions: ReactNode;
  shortcut: ReactNode;
  feedback: ReactNode;
}

export function ModernDlssPanel(props: Props) {
  const { t, language } = useI18n();
  const pt = language === "pt";
  const modelHint = props.modelStatus === "checking" ? (pt ? "Verificando modelos…" : "Checking models…")
    : props.modelStatus === "error" ? (pt ? "Não foi possível verificar os modelos. Reabra os detalhes para tentar novamente." : "Could not check models. Reopen the details to retry.")
    : props.modelStatus === "backend" ? (pt ? "Atualize o backend para habilitar a seleção do modelo novo." : "Update the backend to enable the new model selection.")
    : props.modelStatus === "missing" ? (pt ? "Backend atualizado. Extraia a DLL 310.9.16 para adicionar o modelo novo." : "Backend ready. Extract DLL 310.9.16 to add the new model.")
    : props.model === "original" ? "310.8.0 · " + (pt ? "modelo antigo" : "old model")
    : (pt ? "310.9.16 · modelo novo disponível" : "310.9.16 · new model available");
  const option = (label: string, hint: string, enabled: boolean, onToggle: () => void, disabled = props.busy, description = hint) =>
    <div className="modern-dlss-option"><div><span>{label}</span><HoverTooltip text={hint} tooltipClassName="modern-dlss-tooltip"><p tabIndex={0}>{description}</p></HoverTooltip></div>
      <ModOptionSwitch label={label} enabled={enabled} disabled={disabled} onToggle={onToggle} />
    </div>;
  return <div className="modern-dlss-panels">
    <section className="modern-dlss-card" aria-label={pt ? "Instalação do DLSSNR-AMD" : "DLSSNR-AMD installation"}>
      <h3>{pt ? "Instalar DLSSNR-AMD" : "Install DLSSNR-AMD"}</h3>
      <p className="modern-dlss-description">{props.recommendation || (pt ? "Escolha o modo e a arquitetura para instalar ou reparar o mod neste jogo." : "Choose the mode and architecture to install or repair the mod in this game.")}</p>
      <div className="modern-dlss-setting"><span>{pt ? "Modo" : "Mode"}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={pt ? "Modo de instalação" : "Installation mode"}>
          {(["optiscaler", "reshade", "vulkan", "dx9"] as const).map(value => <HoverTooltip key={value} text={t("routeSelector", `${value}Desc`)} tooltipClassName="modern-dlss-tooltip"><button type="button"
            aria-pressed={props.route === value} disabled={props.busy || (value === "optiscaler" && props.bitness === "32")}
            onClick={() => props.onRoute(value)}>
            {value === "optiscaler" ? "OptiScaler" : value === "reshade" ? "ReShade" : value === "vulkan" ? "Vulkan" : "DX9"}
          </button></HoverTooltip>)}
        </div>{props.recommendOptiscaler && <small className="modern-dlss-recommendation">OptiScaler {pt ? "recomendado" : "recommended"}</small>}</div>
      </div>
      <div className="modern-dlss-setting"><span>{pt ? "Arquitetura" : "Architecture"}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={pt ? "Arquitetura do jogo" : "Game architecture"}>
          {(["64", "32"] as const).map(value => <HoverTooltip key={value} text={value === "32" && props.gpuArch === "rdna3" ? t("bitness", "x86incompat") : ""} tooltipClassName="modern-dlss-tooltip"><button type="button" aria-pressed={props.bitness === value}
            disabled={props.busy || (value === "32" && props.gpuArch === "rdna3")}
            onClick={() => props.onBitness(value)}>{value} bits</button></HoverTooltip>)}
        </div>{props.recommendedBitness && <small className="modern-dlss-recommendation">{props.recommendedBitness} bits {pt ? "recomendado" : "recommended"}</small>}</div>
      </div>
      {(props.modelAvailable || props.unifiedNetworks) && <div className="modern-dlss-setting"><div className="modern-dlss-model-label"><span>{props.unifiedNetworks ? (pt ? "Rede inicial" : "Starting network") : (pt ? "Modelo NR" : "NR model")}</span>
        {props.installed && <small>{props.unifiedNetworks ? (pt ? "No jogo: menu NR ou Ctrl+F11." : "In game: NR menu or Ctrl+F11.") : (pt ? "Após trocar, repare o mod." : "Repair the mod after switching.")}</small>}
      </div>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={pt ? "Modelo neural" : "Neural model"}>
          {(props.unifiedNetworks ? ["small", "original", "int4"] as const : ["small", "original"] as const).map(value => <HoverTooltip key={value}
            text={value === "small" ? "CC_SMALL_FP8 · 310.9.16" : value === "original" ? "CC_SILVER_AARDWOLD · 310.8.0" : "CC_SILVER_AARDWOLD_INT4_MIXED · 310.8.0"} tooltipClassName="modern-dlss-tooltip">
            <button type="button" aria-pressed={value === "int4" ? props.int4 : props.model === value && (!props.unifiedNetworks || !props.int4)}
              disabled={props.busy || (value === "small" && !props.modelAvailable)} onClick={() => {
                if (value === "int4") { props.onModel("original"); if (!props.int4) props.onInt4(); }
                else { props.onModel(value); if (value === "original" && props.int4 && props.unifiedNetworks) props.onInt4(); }
              }}>
              {value === "small" ? (pt ? "Novo" : "New") : value === "original" ? (pt ? "Antigo" : "Old") : "int4"}
            </button>
          </HoverTooltip>)}
        </div><small className="modern-dlss-recommendation">{props.unifiedNetworks && props.int4 ? (pt ? "310.8.0 · int4 misto" : "310.8.0 · mixed int4") : modelHint}</small></div>
      </div>}
      {props.modelStatus === "error" && <p className="modern-dlss-key-hint">{modelHint}</p>}
      <div className="modern-dlss-actions">{props.actions}</div>
    </section>
    <section className="modern-dlss-card" aria-label={pt ? "Opções do DLSSNR-AMD" : "DLSSNR-AMD options"}>
      <h3>{pt ? "Opções" : "Options"}</h3>
      <p className="modern-dlss-description">{pt ? "Em jogos com o mod instalado, Começar ligado e RenoDX são salvos automaticamente para a próxima execução. As demais opções exigem instalar ou reparar." : "For installed mods, Start enabled and RenoDX save automatically for the next launch. Other options require installation or repair."}</p>
      {props.route !== "optiscaler" && props.route !== "remove" && props.lumeniteAvailable && <div className="modern-dlss-setting">
        <span>{pt ? "Vetores de movimento" : "Motion vectors"}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={pt ? "Fonte dos vetores de movimento" : "Motion vector source"}>
          {[false, true].map(enabled => <HoverTooltip key={String(enabled)} tooltipClassName="modern-dlss-tooltip" text={enabled
            ? (pt ? "Ao instalar ou reparar, o backend baixa e instala LumeniteFX do projeto oficial. Depois, alterne pelo menu Add-ons do ReShade. Se incompatível, o backend usa VORT." : "When installing or repairing, the backend downloads and installs LumeniteFX from its official project. Then switch in ReShade's Add-ons menu. If incompatible, the backend falls back to VORT.")
            : (pt ? "Opção padrão, já incluída no backend. Aplicada ao instalar ou reparar." : "Default option, already bundled with the backend. Applied when installing or repairing.")}>
            <button type="button" aria-pressed={props.lumenite === enabled} disabled={props.busy} onClick={() => props.onLumenite(enabled)}>{enabled ? "LumeniteFX" : "VORT"}</button>
          </HoverTooltip>)}
        </div><small className="modern-dlss-recommendation">{props.lumenite ? (pt ? "Download adicional ao instalar / reparar." : "Additional download when installing / repairing.") : (pt ? "Incluído no backend." : "Bundled with the backend.")}</small></div>
      </div>}
      {option(pt ? "Começar ligado" : "Start enabled", pt ? "A caixa do NR já vem marcada ao abrir o jogo." : "The NR checkbox starts checked when launching the game.", props.startup, props.onStartup, props.startupDisabled)}
      {option(t("routeSelector", "renodx"), t("routeSelector", "renodxHint"), props.renodx, props.onRenodx, props.busy || props.startupDisabled, pt ? "Estilo de imagem do add-on DLSS 5." : "Image style from the DLSS 5 add-on.")}
      {!props.unifiedNetworks && option(t("routeSelector", "int4"), t("routeSelector", "int4Hint"), props.int4, props.onInt4, props.busy || props.model === "small", pt ? "Disponível apenas com o modelo antigo (310.8.0)." : "Available only with the old model (310.8.0).")}
      {props.shortcut && <div className="modern-dlss-shortcut"><span>{props.route === "optiscaler" ? (pt ? "Menu do OptiScaler" : "OptiScaler menu") : (pt ? "Menu do ReShade" : "ReShade menu")}</span>{props.shortcut}</div>}
      {props.route === "optiscaler" && <p className="modern-dlss-key-hint">{pt ? "End abre o NR Control. Com RenoDX ativado, os ajustes de NR ficam nesse painel; o menu do OptiScaler continua para upscaling e geração de quadros." : "End opens NR Control. With RenoDX enabled, NR settings move to that panel; OptiScaler keeps its upscaling and frame generation menu."}</p>}
      {props.feedback}
    </section>
  </div>;
}
