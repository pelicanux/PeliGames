import { useI18n } from "../../i18n/I18nContext";

export function DanielGameGuide({ version }: { version: string }) {
  const {t: tx} = useI18n();

  const keys = version === "0.8.0"
    ? [["End", tx("ui-daniel-game-guide", "open_close_the_daniel_panel")], ["PgUp / PgDn", tx("ui-daniel-game-guide", "switch_tabs")], ["↑ / ↓", tx("ui-daniel-game-guide", "select_an_option")], ["← / →", tx("ui-daniel-game-guide", "change_the_value")], ["Backspace", tx("ui-daniel-game-guide", "restore_the_option_default")], ["Page Down", tx("ui-daniel-game-guide", "amdnr_optiscaler_menu_with_the_daniel_panel_closed")]]
    : [["Insert", tx("ui-daniel-game-guide", "open_amdnr_optiscaler_neural")]];
  return <>
    <p className="modern-dlss-description">{tx("ui-daniel-game-guide", "launch_with_proton_and_enable_fsr_dlss_or_xess_in_graphics_settings_the_runtime_needs_a_")}</p>
    <dl className="daniel-nr-shortcuts">{keys.map(([key, text]) => <div key={key}><dt><kbd>{key}</kbd></dt><dd>{text}</dd></div>)}</dl>
    {version === "0.8.0" && <p className="modern-dlss-description daniel-nr-note">{tx("ui-daniel-game-guide", "the_daniel_0_8_panel_uses_keyboard_controls_adjust_the_image_inside_the_game")}</p>}
  </>;
}
