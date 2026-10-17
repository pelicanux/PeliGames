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
    <section className="modern-dlss-card" aria-label={pt ? "Instalação do DLSS 5 NR" : "DLSS 5 NR installation"}>
      <h3>{pt ? "Instalar DLSS 5 NR" : "Install DLSS 5 NR"}</h3>
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
      {props.modelAvailable && <div className="modern-dlss-setting"><span>{pt ? "Modelo NR" : "NR model"}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={pt ? "Modelo neural" : "Neural model"}>
          {(["small", "original"] as const).map(value => <button key={value} type="button" aria-pressed={props.model === value}
            disabled={props.busy || (value === "small" && !props.modelAvailable)} onClick={() => props.onModel(value)}>
            {value === "small" ? (pt ? "Novo" : "New") : (pt ? "Antigo" : "Old")}
          </button>)}
        </div><small className="modern-dlss-recommendation">{modelHint}</small></div>
      </div>}
      {props.modelStatus === "error" && <p className="modern-dlss-key-hint">{modelHint}</p>}
      <div className="modern-dlss-actions">{props.actions}</div>
    </section>
    <section className="modern-dlss-card" aria-label={pt ? "Opções do DLSS 5 NR" : "DLSS 5 NR options"}>
      <h3>{pt ? "Opções" : "Options"}</h3>
      <p className="modern-dlss-description">{pt ? "RenoDX e int4 misto valem na próxima instalação ou reparo." : "RenoDX and mixed int4 apply on the next installation or repair."}</p>
      {option(pt ? "Começar ligado" : "Start enabled", pt ? "A caixa do NR já vem marcada ao abrir o jogo." : "The NR checkbox starts checked when launching the game.", props.startup, props.onStartup, props.startupDisabled)}
      {option(t("routeSelector", "renodx"), t("routeSelector", "renodxHint"), props.renodx, props.onRenodx, props.busy, pt ? "Estilo de imagem do add-on DLSS 5." : "Image style from the DLSS 5 add-on.")}
      {option(t("routeSelector", "int4"), t("routeSelector", "int4Hint"), props.int4, props.onInt4, props.busy || props.model === "small", pt ? "Disponível apenas com o modelo antigo (310.8.0)." : "Available only with the old model (310.8.0).")}
      {props.shortcut && <div className="modern-dlss-shortcut"><span>{props.route === "optiscaler" ? (pt ? "Menu do OptiScaler" : "OptiScaler menu") : (pt ? "Menu do ReShade" : "ReShade menu")}</span>{props.shortcut}</div>}
      {props.route === "optiscaler" && <p className="modern-dlss-key-hint">{pt ? "End abre o NR Control. Com RenoDX ativado, os ajustes de NR ficam nesse painel; o menu do OptiScaler continua para upscaling e geração de quadros." : "End opens NR Control. With RenoDX enabled, NR settings move to that panel; OptiScaler keeps its upscaling and frame generation menu."}</p>}
      {props.feedback}
    </section>
  </div>;
}
