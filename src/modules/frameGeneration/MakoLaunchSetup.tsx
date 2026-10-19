import {PeliRunProfile} from "../../components/PeliRunProfile";
import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {openUrl} from "@tauri-apps/plugin-opener";
import type {GameInfo} from "../../components/GameGrid";
import {MenuIcon} from "../../components/MenuIcon";
import {NexusPopup} from "../nexus/NexusPopup";
import {useI18n} from "../../i18n/I18nContext";

export interface MakoSandboxStatus {app_id:string;runtime:string;branch:string|null;extension_installed:boolean;ready:boolean;message:string;commands:string[];release_url:string}
export function MakoLaunchSetup({game,enabled=true,onClose,onSaved,sandbox,onCheckSandbox}:{game:GameInfo;enabled?:boolean;onClose:()=>void;onSaved:(game:GameInfo)=>void;sandbox?:MakoSandboxStatus;onCheckSandbox?:()=>Promise<void>}) {
 const pt=useI18n().language==="pt";
 const [line,setLine]=useState("");const [error,setError]=useState("");const [busy,setBusy]=useState(false);const [copied,setCopied]=useState(false);
 useEffect(()=>{let live=true;if(!sandbox&&game.launcher==="Steam"&&game.external_id)void invoke<{line:string}>("steam_game_launch_preview",{externalId:game.external_id}).then(r=>{if(live)setLine(r.line)}).catch(e=>{if(live)setError(String(e))});return()=>{live=false}},[game.external_id,sandbox]);
 const copy=async()=>{try{await navigator.clipboard.writeText(sandbox?sandbox.commands.join("\n"):line);setCopied(true)}catch(e){setError(String(e))}};
 const apply=async(action:"restart"|"confirm-manual")=>{setBusy(true);setError("");try{const updated=await invoke<GameInfo>("steam_game_settings_action",{externalId:game.external_id,action});onSaved({...game,...updated});window.dispatchEvent(new Event("peligamesLibraryChanged"));onClose()}catch(e){setError(String(e))}finally{setBusy(false)}};
 const steam=game.launcher==="Steam";
 const title=sandbox?(pt?"Preparar MAKO no Flatpak":"Prepare MAKO in Flatpak"):steam?(pt?"Aplicar opções na Steam":"Apply Steam launch options"):(pt?"Iniciar este jogo com MAKO":"Start this game with MAKO");
 if(!sandbox&&!steam)return <PeliRunProfile game={game} setupOnly onClose={onClose}/>;
 return <NexusPopup className="mako-launch-setup" title={title} titleIcon={<MenuIcon name="settings"/>} onClose={()=>{if(!busy)onClose()}} footer={<>
  <button type="button" className="modern-mini-btn" disabled={busy} onClick={onClose}>{pt?"Fechar":"Close"}</button>
  {sandbox?<button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>{setBusy(true);void onCheckSandbox?.().finally(()=>setBusy(false))}}>{pt?"Verificar preparo":"Verify setup"}</button>:steam?<><button type="button" className="modern-mini-btn" disabled={busy||!line} onClick={()=>void apply("confirm-manual")}>{pt?"Já adicionei · verificar":"I added it · verify"}</button><button type="button" className="modern-mini-btn" disabled={busy} onClick={()=>void apply("restart")}>{busy?(pt?"Aguarde…":"Please wait…"):(pt?"Reiniciar Steam e aplicar":"Restart Steam and apply")}</button></>:null}
  <button type="button" className="modern-mini-btn" disabled={!sandbox&&!line} onClick={()=>void copy()}><MenuIcon name={copied?"check":"copy"}/>{copied?(pt?"Copiado":"Copied"):(pt?"Copiar":"Copy")}</button>
 </>}>
 {sandbox?<>
  <p>{sandbox.message}</p><p><strong>{sandbox.app_id}</strong><br/>{sandbox.runtime}{sandbox.branch?` · ABI ${sandbox.branch}`:""}</p>
  {!sandbox.ready&&sandbox.branch&&<><p>{pt?"1. No lançamento oficial abaixo, baixe MAKO-Renderer-v<versão>-flatpaks.tar.xz da mesma versão selecionada no PeliGames. Extraia o pacote e abra o terminal nessa pasta.":"1. Download MAKO-Renderer-v<version>-flatpaks.tar.xz from the release below, matching the version selected in PeliGames. Extract it and open a terminal in that folder."}</p><p>{pt?"2. Feche o launcher e seus jogos. Os comandos abaixo instalam a extensão do runtime e concedem acesso somente aos perfis e à pasta da DLL. Revise-os antes de executar; o PeliGames não altera permissões automaticamente.":"2. Close the launcher and its games. These commands install the runtime extension and grant access only to profiles and the DLL folder. Review before running; PeliGames does not change permissions automatically."}</p><textarea readOnly rows={5} value={sandbox.commands.join("\n")} aria-label={pt?"Comandos de preparo Flatpak":"Flatpak preparation commands"}/></>}
  <button type="button" className="modern-mini-btn" onClick={()=>void openUrl(sandbox.release_url).catch(e=>setError(String(e)))}>{pt?"Pacotes oficiais do MAKO":"Official MAKO packages"}</button>
  <p>{pt?"3. Clique em Verificar preparo. Depois ative neste jogo e salve. Na Steam Flatpak ainda é necessário adicionar o comando específico nas opções do jogo; o pacote nativo sozinho não ativa a extensão.":"3. Verify setup, then enable this game and save. Steam Flatpak still needs the per-game launch command; the native package alone does not enable the extension."}</p>
 </>:steam?<>
  <p>{enabled?(pt?"Salvar o perfil não ativa o MAKO sozinho. A próxima execução precisa passar pelo comando abaixo.":"Saving a profile alone does not activate MAKO. The next launch must use the command below."):(pt?"A remoção do MAKO também precisa ser aplicada às opções de inicialização da Steam.":"Removing MAKO also requires updating Steam launch options.")}</p>
  <p>{pt?"Steam → Propriedades deste jogo → Geral → Opções de inicialização. Você pode colar a linha com a Steam aberta; feche e abra o jogo depois. Para o PeliGames aplicar automaticamente, use Reiniciar Steam e aplicar.":"Steam → game Properties → General → Launch Options. You can paste the line while Steam is open, then restart the game. Automatic application through PeliGames uses Restart Steam and apply."}</p>
  <textarea readOnly spellCheck={false} rows={4} value={line} aria-label={pt?"Comando de inicialização MAKO na Steam":"MAKO Steam launch command"} onFocus={e=>e.currentTarget.select()}/>
  <p>{pt?"Adicione esta linha uma única vez. O auxiliar lê o perfil atualizado a cada início do jogo, sem reiniciar a Steam. Ativar ou trocar o componente exige reabrir o jogo; multiplicador, escala e modelo leve usam a recarga do MAKO. Preserve seus argumentos adicionais.":"Add this command once. The helper reads your latest profile whenever the game starts, without restarting Steam. Enabling or changing the component requires restarting the game; multiplier, flow scale and lighter-model changes use MAKO's live reload. Preserve additional arguments."}</p>
 </>:null}
 {error&&<p role="alert">{error}</p>}
 </NexusPopup>;
}
