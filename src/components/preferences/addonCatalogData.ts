import {experimentalDanielNr} from "../../services/releaseFeatures";
export const addons = [
  { id: "dlss", label: "DLSSNR-AMD", icon: "graphics", pt: "Prepare o backend e os modelos para Neural Rendering.", en: "Prepare the backend and models for Neural Rendering." },
  { id: "daniel", label: "Daniel NR", icon: "neural", pt: "Prepare as versões experimentais do motor Daniel.", en: "Prepare experimental versions of the Daniel runtime." },
  { id: "nexus", label: "Nexus Mods", icon: "puzzle", pt: "Gerencie backups e recuperação dos seus mods.", en: "Manage backups and recovery for your mods." },
  { id: "mako", label: "MAKO", icon: "bolt", pt: "Instale e gerencie o componente de geração de quadros.", en: "Install and manage the frame generation component." },
  { id: "afmf", label: "AFMF", icon: "graphics", pt: "Prepare o gerador de quadros AFMF para seus jogos.", en: "Prepare AFMF frame generation for your games." },
  { id: "capture", label: "GPU Screen Recorder", icon: "platform", pt: "Prepare gravações de gameplay e replays instantâneos.", en: "Prepare gameplay recording and instant replays." },
] as const;
export type AddonId = typeof addons[number]["id"];
export const addonLabel = (id: AddonId, pt: boolean) => id === "daniel" ? (pt ? "Daniel NR (teste)" : "Daniel NR (test)") : addons.find(addon => addon.id === id)!.label;

export const addonAvailable = (id: AddonId) => id !== "daniel" || experimentalDanielNr;
