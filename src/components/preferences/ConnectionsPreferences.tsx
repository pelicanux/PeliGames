import { useI18n } from "../../i18n/I18nContext";
import { NexusAccountPanel } from "../../modules/nexus/NexusAccountPanel";
import { usePreferencesText } from "./labels";
import { DiscordConnection } from "./DiscordConnection";
import { useState } from "react";
import { MatrixChatPanel } from "../MatrixChatPanel";
import { MatrixIcon } from "../MatrixIcon";
import { MatrixNotificationSoundPreferences } from "./MatrixNotificationSoundPreferences";
import { displayName, useMatrixChat } from "../../services/matrixChat";
import { setMatrixActivityPreferences, useMatrixActivityPreferences } from "../../services/matrixGameActivity";

export function ConnectionsPreferences() {
  const { language } = useI18n();
  const text = usePreferencesText();
  const pt = language === "pt";
  const matrix = useMatrixChat();
  const activity = useMatrixActivityPreferences();
  const [matrixOpen, setMatrixOpen] = useState(false);
  return <>
    {matrixOpen && <MatrixChatPanel onClose={() => setMatrixOpen(false)}/>}
    <h2>{text.connections}</h2>
    <p>{pt ? "Gerencie suas contas e integrações com a comunidade." : "Manage your accounts and community integrations."}</p>
    <div className="preference-connections">
      <section className="preference-connection" data-preference-id="connection-nexus">
        <header><img src="/nexus-mods.svg" alt=""/><h3>Nexus Mods</h3></header>
        <NexusAccountPanel/>
      </section>
      <DiscordConnection/>
      <section className="preference-connection" data-preference-id="connection-matrix">
        <header><MatrixIcon/><h3>Matrix</h3><span className="preference-connection-status">{matrix.userId ? pt ? "Conectado" : "Connected" : matrix.pending ? pt ? "Aguardando login" : "Waiting for sign-in" : pt ? "Não conectado" : "Not connected"}</span></header>
        <p>{pt ? "Um chat retrô para conversar sobre jogos, mods e Linux. Conecte sua conta pelo navegador dentro do chat." : "A retro chat about games, mods and Linux. Connect your account through the browser from the chat."}</p>
        {matrix.userId && <p><strong>{displayName(matrix.userId, matrix.names)}</strong></p>}
        <label className="preference-switch"><input type="checkbox" checked={activity.sharing} onChange={event => setMatrixActivityPreferences({ sharing: event.target.checked })}/><span className="preference-switch-track"/><span>{pt ? "Compartilhar atividade de jogo no Matrix" : "Share game activity on Matrix"}<small>{pt ? "Mostra o jogo em execução para seus contatos, quando o servidor oferece presença." : "Shows your running game to contacts when the server supports presence."}</small></span></label>
        <label className="preference-switch"><input type="checkbox" checked={activity.covers} onChange={event => setMatrixActivityPreferences({ covers: event.target.checked })}/><span className="preference-switch-track"/><span>{pt ? "Mostrar capas nas atividades" : "Show covers in activities"}<small>{pt ? "Exibe uma capa pequena no chat do PeliGames. Desative para mostrar apenas o texto." : "Displays a small cover in PeliGames chat. Disable for text only."}</small></span></label>
        {activity.error && <p role="status">{activity.error}</p>}
        <MatrixNotificationSoundPreferences/>
        <button type="button" className="btn btn-primary" onClick={() => setMatrixOpen(true)}><MatrixIcon/>{pt ? "Abrir chat Matrix" : "Open Matrix chat"}</button>
      </section>
    </div>
  </>;
}
