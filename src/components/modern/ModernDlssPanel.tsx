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
  model: "small" | "experimental" | "original";
  experimentalAvailable?: boolean;
  nrPlugin?: boolean;
  modelAvailable: boolean;
  unifiedNetworks: boolean;
  lumeniteAvailable: boolean;
  lumenite: boolean;
  onLumenite: (enabled: boolean) => void;
  installed?: boolean;
  modelStatus: "checking" | "error" | "backend" | "missing" | "ready";
  onModel: (model: "small" | "experimental" | "original") => void;
  int4: boolean;
  onInt4: () => void;
  actions: ReactNode;
  shortcut: ReactNode;
  feedback: ReactNode;
}

export function ModernDlssPanel(props: Props) {

  const { t } = useI18n();

  const modelHint = props.modelStatus === "checking" ? (t("ui-modern-dlss-panel", "checking_models"))
    : props.modelStatus === "error" ? (t("ui-modern-dlss-panel", "could_not_check_models_reopen_the_details_to_retry"))
    : props.modelStatus === "backend" ? (t("ui-modern-dlss-panel", "update_the_backend_to_enable_the_new_model_selection"))
    : props.modelStatus === "missing" ? (t("ui-modern-dlss-panel", "backend_ready_extract_dll_310_9_16_to_add_the_new_model"))
    : props.model === "experimental" ? (t("ui-modern-dlss-panel", "derived_from_310_9_16_faster_may_change_the_image"))
    : props.model === "original" ? "310.8.0 · " + (t("ui-modern-dlss-panel", "old_model"))
    : (t("ui-modern-dlss-panel", "310_9_16_new_model_available"));
  const option = (label: string, hint: string, enabled: boolean, onToggle: () => void, disabled = props.busy, description = hint) =>
    <div className="modern-dlss-option"><div><span>{label}</span><HoverTooltip text={hint} tooltipClassName="modern-dlss-tooltip"><p tabIndex={0}>{description}</p></HoverTooltip></div>
      <ModOptionSwitch label={label} enabled={enabled} disabled={disabled} onToggle={onToggle} />
    </div>;
  return <div className="modern-dlss-panels">
    <section className="modern-dlss-card" aria-label={t("ui-modern-dlss-panel", "dlssnr_amd_installation")}>
      <h3>{t("ui-modern-dlss-panel", "install_dlssnr_amd")}</h3>
      <p className="modern-dlss-description">{props.recommendation || (t("ui-modern-dlss-panel", "choose_the_mode_and_architecture_to_install_or_repair_the_mod_in_this_game"))}</p>
      <div className="modern-dlss-setting"><span>{t("ui-modern-dlss-panel", "mode")}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={t("ui-modern-dlss-panel", "installation_mode")}>
          {(["optiscaler", "reshade", "vulkan", "dx9"] as const).map(value => <HoverTooltip key={value} text={value === "optiscaler" && props.nrPlugin ? (t("ui-modern-dlss-panel", "original_optiscaler_with_nr_plugin_dx11_dx12_and_vulkan_64_bit_requires_dlss_fsr_or_xess")) : t("routeSelector", `${value}Desc`)} tooltipClassName="modern-dlss-tooltip"><button type="button"
            aria-pressed={props.route === value} disabled={props.busy || (value === "optiscaler" && props.bitness === "32")}
            onClick={() => props.onRoute(value)}>
            {value === "optiscaler" ? "OptiScaler" : value === "reshade" ? "ReShade" : value === "vulkan" ? "Vulkan" : "DX9"}
          </button></HoverTooltip>)}
        </div>{props.recommendOptiscaler && <small className="modern-dlss-recommendation">OptiScaler {t("ui-modern-dlss-panel", "recommended")}</small>}</div>
      </div>
      <div className="modern-dlss-setting"><span>{t("ui-modern-dlss-panel", "architecture")}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={t("ui-modern-dlss-panel", "game_architecture")}>
          {(["64", "32"] as const).map(value => <HoverTooltip key={value} text={value === "32" && props.gpuArch === "rdna3" ? t("bitness", "x86incompat") : ""} tooltipClassName="modern-dlss-tooltip"><button type="button" aria-pressed={props.bitness === value}
            disabled={props.busy || (value === "32" && props.gpuArch === "rdna3")}
            onClick={() => props.onBitness(value)}>{value} bits</button></HoverTooltip>)}
        </div>{props.recommendedBitness && <small className="modern-dlss-recommendation">{props.recommendedBitness} bits {t("ui-modern-dlss-panel", "recommended")}</small>}</div>
      </div>
      {(props.modelAvailable || props.unifiedNetworks) && <div className="modern-dlss-setting"><div className="modern-dlss-model-label"><span>{props.unifiedNetworks ? (t("ui-modern-dlss-panel", "starting_network")) : (t("ui-modern-dlss-panel", "nr_model"))}</span>
        {props.installed && <small>{props.unifiedNetworks ? (t("ui-modern-dlss-panel", "in_game_nr_menu_or_ctrl_f11")) : (t("ui-modern-dlss-panel", "repair_the_mod_after_switching"))}</small>}
      </div>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={t("ui-modern-dlss-panel", "neural_model")}>
          {(props.unifiedNetworks ? ["small", ...(props.experimentalAvailable ? ["experimental" as const] : []), "original", "int4"] as const : ["small", "original"] as const).map(value => <HoverTooltip key={value}
            text={value === "experimental" ? "CC_SMALL_FP8_EXPERIMENTAL · 310.9.16" : value === "small" ? "CC_SMALL_FP8 · 310.9.16" : value === "original" ? "CC_SILVER_AARDWOLD · 310.8.0" : "CC_SILVER_AARDWOLD_INT4_MIXED · 310.8.0"} tooltipClassName="modern-dlss-tooltip">
            <button type="button" aria-pressed={value === "int4" ? props.int4 : props.model === value && (!props.unifiedNetworks || !props.int4)}
              disabled={props.busy || ((value === "small" || value === "experimental") && !props.modelAvailable)} onClick={() => {
                if (value === "int4") { props.onModel("original"); if (!props.int4) props.onInt4(); }
                else { props.onModel(value); if (value === "original" && props.int4 && props.unifiedNetworks) props.onInt4(); }
              }}>
              {value === "small" ? (t("ui-modern-dlss-panel", "new")) : value === "experimental" ? "Experimental" : value === "original" ? (t("ui-modern-dlss-panel", "old")) : "int4"}
            </button>
          </HoverTooltip>)}
        </div><small className="modern-dlss-recommendation">{props.unifiedNetworks && props.int4 ? (t("ui-modern-dlss-panel", "310_8_0_mixed_int4")) : modelHint}</small></div>
      </div>}
      {props.modelStatus === "error" && <p className="modern-dlss-key-hint">{modelHint}</p>}
      <div className="modern-dlss-actions">{props.actions}</div>
      {props.nrPlugin && <p className="modern-dlss-key-hint">{t("ui-modern-dlss-panel", "install_repair_replaces_the_game_s_nr_settings_with_backend_defaults_and_the_choices_abo")}</p>}
    </section>
    <section className="modern-dlss-card" aria-label={t("ui-modern-dlss-panel", "dlssnr_amd_options")}>
      <h3>{t("ui-modern-dlss-panel", "options")}</h3>
      <p className="modern-dlss-description">{t("ui-modern-dlss-panel", "for_installed_mods_start_enabled_and_renodx_save_automatically_for_the_next_launch_other")}</p>
      {props.route !== "optiscaler" && props.route !== "remove" && props.lumeniteAvailable && <div className="modern-dlss-setting">
        <span>{t("ui-modern-dlss-panel", "motion_vectors")}</span>
        <div className="modern-dlss-choice"><div className="modern-dlss-segments" role="group" aria-label={t("ui-modern-dlss-panel", "motion_vector_source")}>
          {[false, true].map(enabled => <HoverTooltip key={String(enabled)} tooltipClassName="modern-dlss-tooltip" text={enabled
            ? (t("ui-modern-dlss-panel", "when_installing_or_repairing_the_backend_downloads_and_installs_lumenitefx_from_its_offi"))
            : (t("ui-modern-dlss-panel", "default_option_already_bundled_with_the_backend_applied_when_installing_or_repairing"))}>
            <button type="button" aria-pressed={props.lumenite === enabled} disabled={props.busy} onClick={() => props.onLumenite(enabled)}>{enabled ? "LumeniteFX" : "VORT"}</button>
          </HoverTooltip>)}
        </div><small className="modern-dlss-recommendation">{props.lumenite ? (t("ui-modern-dlss-panel", "additional_download_when_installing_repairing")) : (t("ui-modern-dlss-panel", "bundled_with_the_backend"))}</small></div>
      </div>}
      {option(t("ui-modern-dlss-panel", "start_enabled"), t("ui-modern-dlss-panel", "the_nr_checkbox_starts_checked_when_launching_the_game"), props.startup, props.onStartup, props.startupDisabled)}
      {option(t("routeSelector", "renodx"), t("routeSelector", "renodxHint"), props.renodx, props.onRenodx, props.busy || props.startupDisabled, t("ui-modern-dlss-panel", "image_style_from_the_dlss_5_add_on"))}
      {!props.unifiedNetworks && option(t("routeSelector", "int4"), t("routeSelector", "int4Hint"), props.int4, props.onInt4, props.busy || props.model === "small", t("ui-modern-dlss-panel", "available_only_with_the_old_model_310_8_0"))}
      {props.shortcut && <div className="modern-dlss-shortcut"><span>{props.route === "optiscaler" ? (t("ui-modern-dlss-panel", "optiscaler_menu")) : (t("ui-modern-dlss-panel", "reshade_menu"))}</span>{props.shortcut}</div>}
      {props.route === "optiscaler" && <p className="modern-dlss-key-hint">{props.nrPlugin ? (t("ui-modern-dlss-panel", "in_game_press_end_to_adjust_nr_in_nr_control_with_backend_0_0_8_1_or_newer_customize_key")) : (t("ui-modern-dlss-panel", "end_opens_nr_control_with_renodx_enabled_nr_settings_move_to_that_panel_optiscaler_keeps"))}</p>}
      {props.route !== "optiscaler" && props.route !== "remove" && <p className="modern-dlss-key-hint">{t("ui-modern-dlss-panel", "in_game_adjust_nr_in_the_reshade_menu_s_add_ons_tab")}</p>}
      {props.feedback}
    </section>
  </div>;
}
