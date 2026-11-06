import type {ReactNode} from "react";
import type {GameInfo} from "../../components/GameGrid";
import type {GameAnalysisResult} from "../../hooks/useGameAnalysis";
import type {GameCopyReference} from "../../hooks/useGameCopyReference";
import type {GamePanelMode} from "../../types/gamePanel";
import {useI18n} from "../../i18n/I18nContext";
import {isInstalledMod} from "../../services/installationStatus";
import {getUpscalerFamilies, isKnownGameInfo} from "./gameInfoPresentation";
import {GamePathTile} from "./GamePathTile";

interface Props {
  selectedGame: GameInfo;
  gamePanelMode: GamePanelMode;
  info: GameAnalysisResult;
  nexusGame?: {proton?: string; platform?: string; compat_data?: string; profile?: string | null; game: {executable?: string | null}};
  selectedGameRoot?: string;
  releaseDate: string;
  installStatus: string;
  installedDll: string | null;
  copyReference: GameCopyReference | null;
  modPathsChecked: boolean;
  modPaths?: {dlss_destination?: string};
  directoryPickerBusy: boolean;
  onChooseCustomPath: () => void | Promise<void>;
  onDuplicate: (game: GameInfo) => void;
}
export function SelectedGameOverview({selectedGame, gamePanelMode, info, nexusGame, selectedGameRoot, releaseDate, installStatus, installedDll, copyReference, modPathsChecked, modPaths, directoryPickerBusy, onChooseCustomPath, onDuplicate}: Props) {
  const {t} = useI18n();
  const localizeAnalysisValue = (value: string) => {
    if (value === "Linux Nativo" || value === "Native Linux") return t("gameInfo", "nativeLinux");
    if (["Não detectada", "Não detectado", "Not detected"].includes(value)) return t("gameInfo", "notDetected");
    if (["Verificando...", "Checking..."].includes(value)) return t("gameInfo", "verifying");
    if (["Desconhecido", "Desconhecida", "Unknown"].includes(value)) return t("gameInfo", "unknown");
    return value;
  };
  const infoFamilies = getUpscalerFamilies(info.upscalers);
  const checking = (value: string) => value === "Verificando..." ? "…" : localizeAnalysisValue(value);
  const selectedProton = gamePanelMode === "nexus" ? nexusGame?.proton : selectedGame?.proton;
  const nexusPlatform = nexusGame ? (nexusGame.platform === "native" ? t("modern", "nativeLinux") : "Windows") : "";
  const tile = (label: string, value: ReactNode, wide = false) => <div className={`modern-tile${wide ? " wide" : ""}`}><small>{label}</small><strong>{value}</strong></div>;
  const overviewPrefix = gamePanelMode === "nexus" ? nexusGame?.compat_data || selectedGame?.prefix : selectedGame?.prefix;
  const overviewExecutable = (gamePanelMode === "nexus" ? nexusGame?.game.executable : selectedGame?.executable)?.trim();
  const overviewExecutableFolder = overviewExecutable && /[\\/]/.test(overviewExecutable)
    ? overviewExecutable.replace(/[\\/][^\\/]*$/, "") || "/" : undefined;
  const pathTile = (label: string, path?: string, action?: ReactNode) => <GamePathTile label={label} path={path} action={action}/>;
  return <div className="modern-tiles modern-overview-tiles">
    {isKnownGameInfo(gamePanelMode === "nexus" ? nexusPlatform : info.platform) && tile(t("modern", "platform"), gamePanelMode === "nexus" ? nexusPlatform : /^windows\b/i.test(info.platform) ? "Windows" : checking(info.platform))}
    {isKnownGameInfo(info.graphics_api) && tile(t("modern", "graphicsApi"), checking(info.graphics_api))}
    {isKnownGameInfo(info.architecture) && tile(t("modern", "architecture"), checking(info.architecture))}
    {infoFamilies.filter(isKnownGameInfo).length > 0 && tile(t("modern", "upscalers"), infoFamilies.filter(isKnownGameInfo).join(" · "))}
    {isKnownGameInfo(selectedProton) && tile("Proton", <span title={selectedProton}>{selectedProton?.split("/").filter(Boolean).pop()}</span>)}
    {isKnownGameInfo(releaseDate) && tile(t("gameInfo", "release"), releaseDate)}
    {isInstalledMod(installStatus) && isKnownGameInfo(installedDll) && tile(t("gameInfo", "injectedDll"), <code>{installedDll}</code>)}
    {gamePanelMode === "mods" && isInstalledMod(installStatus) && tile(t("gameInfo", "modInstalled"), installStatus)}
    {isKnownGameInfo(nexusGame?.profile) && tile(t("ui-app", "mod_profile"), nexusGame?.profile)}
    {copyReference?.reserved && <div className="modern-tile"><h3>Base reservada · {copyReference.dependents.length} overlay(s)</h3><p>Esta instalação está reservada para as cópias: {copyReference.dependents.join(", ")}. Execute uma cópia. Ao remover o último overlay, a base volta a ficar disponível. Não altere nem mova seus arquivos por fora do PeliGames.</p><button className="modern-mini-btn" onClick={()=>{onDuplicate(selectedGame);}}>Criar outra cópia</button></div>}
    {copyReference?.kind === "reflink" && <div className="modern-tile"><h3>Cópia independente · reflink</h3><p>Compartilha espaço até os arquivos serem alterados. Não depende da instalação original.</p></div>}
    <div className="modern-overview-paths">
      {copyReference?.base && pathTile("Prefixo de referência · base reservada",copyReference.base)}
      {copyReference?.reserved && pathTile("Prefixo de referência · base reservada",selectedGame.prefix)}
      {pathTile(`${t("modern", "gameFolder")} · ${selectedGame?.launcher || ""}`, selectedGameRoot, gamePanelMode === "mods" && <button type="button" className="modern-mini-btn" disabled={directoryPickerBusy} onClick={() => void onChooseCustomPath()} title={t("gameInfo", "editDirectory")}>{t("modern", "change")}</button>)}
      {!modPathsChecked && tile("DLSSNR-AMD", t("ui-app", "destination_not_checked_yet_open_the_dlssnr_amd_tab"))}
      {pathTile(t("ui-app", "dlssnr_amd_destination_executable_folder"), modPathsChecked ? modPaths?.dlss_destination : undefined)}
      {pathTile(t("ui-app", "prefix_folder"), overviewPrefix)}
      {pathTile(t("ui-app", "executable_folder"), overviewExecutableFolder)}
    </div>
  </div>;
}
