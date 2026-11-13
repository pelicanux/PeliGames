import type {AddonHost, AddonInstance, AddonViewProps} from "../../../ui-sdk";

/** Type-only SDK import: the distributed module has no second copy of React. */
export function createAddon(host: AddonHost): AddonInstance {
  const {React, ui} = host;
  const t = host.i18n.createTranslator({defaultLanguage: "en", resources: {
    en: {
      option: "Example option", hint: "Demonstrates a shared launcher control.",
      test: "Test service", unavailable: "Service unavailable",
      idle: "The service starts when the button is clicked.", updates: "UI updates",
    },
    "pt-BR": {
      option: "Exemplo de opção", hint: "Demonstra o controle compartilhado do launcher.",
      test: "Testar serviço", unavailable: "Serviço indisponível",
      idle: "O serviço inicia ao clicar no botão.", updates: "Atualizações da interface",
    },
  }});
  function Preferences({disabled}: AddonViewProps) {
    const [enabled, setEnabled] = React.useState(false);
    const [reply, setReply] = React.useState("");
    const [ticks, setTicks] = React.useState(0);
    React.useEffect(() => host.lifecycle.interval(() => setTicks(value => value + 1), 1000), []);
    return <div>
      <ui.Row icon="settings" label={t("option")} hint={t("hint")}>
        <ui.Switch label={t("option")} enabled={enabled} disabled={disabled} onToggle={() => setEnabled(value => !value)}/>
      </ui.Row>
      <ui.Button disabled={disabled} onClick={() => {
        void host.service.call("echo", {enabled}).then(value => {if (!host.lifecycle.signal.aborted) setReply(JSON.stringify(value))}, () => {if (!host.lifecycle.signal.aborted) setReply(t("unavailable"))});
      }}>{t("test")}</ui.Button>
      <p role="status">{reply || t("idle")}</p>
      <small>{t("updates")}: {ticks}</small>
    </div>;
  }
  return {apiVersion: 1, id: host.addon.id, version: host.addon.version, views: {preferences: Preferences}, dispose() {}};
}
