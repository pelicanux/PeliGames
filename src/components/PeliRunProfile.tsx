import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { GameInfo } from "./GameGrid";
import { normalizeAdvanced } from "../services/advancedSettings";
import { useI18n } from "../i18n/I18nContext";
import { notifyError } from "../services/notifications";
import { NexusPopup } from "../modules/nexus/NexusPopup";
import { MenuIcon } from "./MenuIcon";

interface ProfileExport { id: string; executable: string; arguments: string; line: string }
type Target = "Steam" | "Heroic" | "Lutris" | "Outro";

/** Exports a wrapper; never writes another launcher's configuration or permissions. */
export function PeliRunProfile({ game, disabled, setupOnly = false, inline = false, onClose }: { game: GameInfo; disabled?: boolean; setupOnly?: boolean; inline?: boolean; onClose?: () => void }) {
  const pt = useI18n().language === "pt";
  const [profile, setProfile] = useState<ProfileExport | null>(null);
  const [busy, setBusy] = useState(false);
  const [target, setTarget] = useState<Target>(["Steam", "Heroic", "Lutris"].includes(game.launcher) ? game.launcher as Target : "Outro");
  const [copied, setCopied] = useState("");
  const open = async () => {
    if (busy || disabled) return;
    setBusy(true);
    try {
      setProfile(await invoke<ProfileExport>("export_launch_profile", { name: game.name, path: game.path, externalId: game.external_id ?? null, advanced: normalizeAdvanced(game.advanced) }));
      setCopied("");
    } catch (error) { notifyError("PeliRun", String(error)); }
    finally { setBusy(false); }
  };
  useEffect(() => { if (setupOnly) void open(); }, [setupOnly]); // The saved game is fixed for this setup dialog.
  const copy = async (key: string, value: string) => {
    try { await navigator.clipboard.writeText(value); setCopied(key); }
    catch (error) { notifyError("PeliRun", String(error)); }
  };
  const field = (label: string, key: string, value: string) => <label className="pelirun-export-field"><span>{label}</span><div><input aria-label={label} readOnly value={value} onFocus={event => event.target.select()} /><button type="button" className="modern-mini-btn" onClick={() => void copy(key, value)}><MenuIcon name="copy" />{copied === key ? (pt ? "Copiado" : "Copied") : (pt ? "Copiar" : "Copy")}</button></div></label>;
  const details = profile && <>
      <p>{pt ? "Selecione onde você vai iniciar este jogo. O identificador e o perfil são os mesmos em todos os launchers." : "Select where you will launch this game. The identifier and profile stay the same across launchers."}</p>
      <div className="pelirun-export-targets" role="group" aria-label={pt ? "Launcher de destino" : "Target launcher"}>{(["Steam", "Heroic", "Lutris", "Outro"] as Target[]).map(item => <button key={item} type="button" className="modern-mini-btn" aria-pressed={target === item} onClick={() => { setTarget(item); setCopied(""); }}>{item === "Outro" && !pt ? "Other" : item}</button>)}</div>
      <p>{target === "Steam" ? (pt ? "Propriedades do jogo → Geral → Opções de inicialização. Use a linha abaixo e mantenha a seleção do Proton na Steam." : "Game properties → General → Launch options. Use the line below and keep Proton selection in Steam.") : target === "Heroic" ? (pt ? "Configurações do jogo → Avançado → Comando wrapper. Informe o executável e os argumentos nos respectivos campos." : "Game settings → Advanced → Wrapper command. Enter the executable and arguments in their respective fields.") : target === "Lutris" ? (pt ? "Configurar jogo → Opções do sistema → Prefixo de comando. Use a linha abaixo." : "Configure game → System options → Command prefix. Use the line below.") : (pt ? "Use o campo de wrapper ou prefixo de comando do launcher. Ele deve passar o comando completo do jogo ao PeliRun; um campo que aceita apenas argumentos do jogo não é suficiente." : "Use the launcher's wrapper or command prefix setting. It must pass the complete game command to PeliRun; a game-arguments-only field is insufficient.")}</p>
      {target === "Heroic" ? <>{field(pt ? "Executável" : "Executable", "binary", profile.executable)}{field(pt ? "Argumentos do wrapper" : "Wrapper arguments", "args", profile.arguments)}</> : field(pt ? "Comando" : "Command", "line", `${profile.line}${target === "Steam" ? " %command%" : ""}`)}
      <p className="pelirun-export-note">{pt ? "Configure uma vez. Depois, salve as alterações no PeliGames e abra o jogo novamente. Isso não troca automaticamente o Proton escolhido no outro launcher." : "Set up once. Then save changes in PeliGames and relaunch the game. This does not automatically change the other launcher's selected Proton."}</p>
      <p className="pelirun-export-note">{pt ? "Flatpak: o launcher precisa ter acesso ao PeliRun, ao perfil e aos componentes usados pelo jogo. Nenhuma permissão é alterada automaticamente." : "Flatpak: the launcher needs access to PeliRun, its profile, and the game's components. No permissions are changed automatically."}</p>
  </>;
  return <section className="pelirun-export" style={inline ? { display: "block" } : undefined}>
    {!setupOnly && <><div><strong>PeliRun</strong><p>{pt ? "Use este perfil em outro launcher. As alterações salvas aqui valem na próxima abertura do jogo." : "Use this profile in another launcher. Settings saved here apply on the next game launch."}</p></div>
    <button type="button" className="modern-mini-btn" disabled={disabled || busy} onClick={() => void open()}>{busy ? (pt ? "Preparando…" : "Preparing…") : (pt ? "Usar em outro launcher" : "Use in another launcher")}</button></>}
    {setupOnly && !profile && <div><p>{busy ? (pt ? "Preparando perfil PeliRun…" : "Preparing PeliRun profile…") : (pt ? "Não foi possível preparar o perfil. Tente novamente." : "Could not prepare the profile. Try again.")}</p><button type="button" className="modern-mini-btn" disabled={busy} onClick={() => void open()}>{pt ? "Tentar novamente" : "Retry"}</button>{!inline && <button type="button" className="modern-mini-btn" onClick={onClose}>{pt ? "Fechar" : "Close"}</button>}</div>}
    {profile && (inline ? details : <NexusPopup title={`PeliRun · ${profile.id}`} onClose={() => { setProfile(null); onClose?.(); }}>{details}</NexusPopup>)}
  </section>;
}
