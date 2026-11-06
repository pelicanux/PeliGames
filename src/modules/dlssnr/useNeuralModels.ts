import {modelSourcePaths} from "../../services/modConfiguration";
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "../../types/appConfig";

interface NeuralModelStatus {
  small: boolean;
  original: boolean;
  selectable: boolean;
  unified_networks: boolean;
  lumenite: boolean;
  lumenite_selected: boolean;
  experimental: boolean;
  nr_plugin: boolean;
  selected_network: string | null;
  chain_installed: boolean;
}
interface Context {
  active: boolean;
  gamePath?: string;
  gameDir: string;
  config: AppConfig | null;
  gpuArch: "rdna3" | "rdna4";
  bitness: "32" | "64";
  paused: boolean;
}

export function useNeuralModels({ active: enabled, gamePath, gameDir, config, gpuArch, bitness, paused }: Context) {
  const [neuralModel, setNeuralModel] = useState<"small" | "experimental" | "original">("small");
  const neuralNetworkIdentity=useRef("");
  const [neuralModelsError, setNeuralModelsError] = useState(false);
  const [neuralModels, setNeuralModels] = useState<NeuralModelStatus | null>(null);
  const [lumeniteChoices, setLumeniteChoices] = useState<Record<string, boolean>>({});
  const [int4Mixed, setInt4Mixed] = useState(false);
  useEffect(() => {
    if (!enabled || !gamePath || !config?.dll_version || paused) return;
    let active = true, revision = 0;
    const refreshModels = async () => {
      const current = ++revision;
      setNeuralModelsError(false);
      try {
        const status = await invoke<NeuralModelStatus>("get_neural_model_status", {
          gpuArch, bitness, gameDir, binPath: modelSourcePaths(config).binPath,
        });
        if (!active || revision !== current) return;
        setNeuralModels(status);
        if(neuralNetworkIdentity.current!==gameDir){
          neuralNetworkIdentity.current=gameDir;
          const small=status.small && status.selectable;
          setNeuralModel(status.selected_network==="CC_SMALL_FP8_EXPERIMENTAL" && status.experimental && small ? "experimental"
            : status.selected_network?.startsWith("CC_SILVER_AARDWOLD") || !small ? "original" : "small");
          setInt4Mixed(status.selected_network==="CC_SILVER_AARDWOLD_INT4_MIXED");
        }else setNeuralModel(previous => !status.small || !status.selectable ? "original" : previous === "experimental" && !status.experimental ? "small" : previous);
      } catch {
        if (!active || revision !== current) return;
        setNeuralModels(null); setNeuralModelsError(true);
      }
    };
    const onFocus = () => { void refreshModels(); };
    const onBackendChanged = (event: Event) => {
      const target = (event as CustomEvent<{target?:string}>).detail?.target;
      if (!target || target === gpuArch) void refreshModels();
    };
    setNeuralModels(null);
    void refreshModels();
    window.addEventListener("focus", onFocus);
    window.addEventListener("backendVersionChecked", onBackendChanged);
    return () => {
      active = false; ++revision;
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("backendVersionChecked", onBackendChanged);
    };
    // A new extraction can keep exactly the same dll_version pathname. Recheck
    // the saved configuration object and dialog lifecycle, not just that string.
  }, [enabled, gamePath, gameDir, config, gpuArch, bitness, paused]);
  return { neuralModel, setNeuralModel, neuralModels, neuralModelsError, lumeniteChoices, setLumeniteChoices, int4Mixed, setInt4Mixed };
}
