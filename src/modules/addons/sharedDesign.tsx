import {MangoHudShortcutRecorder} from "../../components/preferences/MangoHudShortcutRecorder";
import * as React from "react";
import {openFilePicker,openDirectoryPicker} from "../../services/tauriService";
import {invoke} from "@tauri-apps/api/core";
import type {AddonHost, AddonUi} from "../../../addons/_infra/ui-sdk";
import {SettingsCategories} from "../../components/SettingsCategories";
import {ModOptionRow, ModOptionSwitch} from "../../components/ModOptionRow";
import {SelectionMenu} from "../../components/SelectionMenu";
import {MenuIcon} from "../../components/MenuIcon";
import {GamePathTile} from "../launcher/GamePathTile";
import {ComponentDownloadProgress} from "../../components/ComponentDownloadProgress";
import {AddonProjectLinks} from "../../components/preferences/AddonProjectLinks";
import {AddonLaunchProfile} from "./gameHost";

const ui: AddonUi = Object.freeze<AddonUi>({
  Shortcut:MangoHudShortcutRecorder,
  Categories: SettingsCategories<string>,
  Row: ModOptionRow,
  Switch: props => <ModOptionSwitch {...props} appearance="rail"/>,
  Choice: SelectionMenu,
  Button: ({primary, ...props}) => <button {...props} type="button" className={primary ? "mod-action-primary" : "mod-action-quiet"}/>,
  Icon: MenuIcon,
  Path: GamePathTile,
  Progress: ComponentDownloadProgress,
  Projects: AddonProjectLinks,
  LaunchProfile: AddonLaunchProfile,
});
/** All external views use the launcher's existing React and controls. */
export const addonDesign: Pick<AddonHost, "React" | "ui" | "files"> = Object.freeze({React, ui,
  files: Object.freeze({openFile:(path:string)=>invoke<void>("addon_open_file",{path}),machine:()=>invoke<{gpus:{model:string;pciAddress:string;primary:boolean}[];steam_libraries:string[]}>("addon_machine_context"),chooseFile:openFilePicker,chooseDirectory:openDirectoryPicker,openDirectory: (path: string) => invoke<void>("open_folder", {path})}),
});
