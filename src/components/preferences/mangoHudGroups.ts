/** UI grouping only: native option names and saved values remain unchanged. */
export const mangoHudGroups: Record<string, { title: string; keys: string[] }[]> = {
  visual: [
    { title: "Layout e orientação", keys: ["horizontal", "horizontal_stretch", "hud_compact", "table_columns"] },
    { title: "Fundo e bordas", keys: ["background_color", "background_alpha", "alpha", "round_corners"] },
    { title: "Texto e fonte", keys: ["custom_text_center", "font_size", "text_color", "text_outline"] },
    { title: "Posição e deslocamento", keys: ["position", "offset_x", "offset_y"] },
    { title: "Cores das métricas", keys: ["gpu_color", "cpu_color", "ram_color", "vram_color", "frametime_color"] },
    { title: "Visibilidade e atalho", keys: ["no_display", "toggle_hud"] },
  ],
  fps: [
    { title: "Informações de desempenho", keys: ["fps", "frametime", "fps_metrics", "frame_count", "frame_timing"] },
    { title: "Limitador de FPS", keys: ["fps_limit", "fps_limit_method", "show_fps_limit"] },
    { title: "Sincronização vertical", keys: ["vsync", "gl_vsync"] },
    { title: "Cores por faixa de FPS", keys: ["fps_color_change", "fps_value", "fps_color"] },
  ],
  gpu: [
    { title: "Métricas principais", keys: ["gpu_stats", "gpu_core_clock", "gpu_mem_clock", "vram", "proc_vram"] },
    { title: "Temperaturas e ventoinhas", keys: ["gpu_temp", "gpu_mem_temp", "gpu_junction_temp", "gpu_fan"] },
    { title: "Energia e limitações", keys: ["gpu_power", "gpu_voltage", "gpu_power_limit", "throttling_status", "throttling_status_graph"] },
    { title: "Informações da GPU", keys: ["gpu_text", "gpu_name", "vulkan_driver", "gpu_list"] },
  ],
  cpu: [
    { title: "Identificação da CPU", keys: ["cpu_text"] },
    { title: "Métricas principais", keys: ["cpu_stats", "core_load", "cpu_mhz", "core_type"] },
    { title: "Temperatura e energia", keys: ["cpu_temp", "cpu_power"] },
    { title: "Memória", keys: ["ram", "swap", "procmem"] },
    { title: "Leitura e escrita", keys: ["io_read", "io_write"] },
  ],
  extras: [
    { title: "Sistema", keys: ["resolution", "display_server", "time", "arch"] },
    { title: "Wine e API gráfica", keys: ["wine", "engine_version", "winesync"] },
    { title: "Ferramentas de execução", keys: ["version", "gamemode", "vkbasalt"] },
    { title: "Bateria", keys: ["battery", "battery_watt", "battery_time"] },
    { title: "Outras informações", keys: ["media_player", "network", "temp_fahrenheit"] },
  ],
  logs: [
    { title: "Coleta de dados", keys: ["log_duration", "autostart_log", "log_interval"] },
    { title: "Controle do registro", keys: ["toggle_logging"] },
  ],
};
