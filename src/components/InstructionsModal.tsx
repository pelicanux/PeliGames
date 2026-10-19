import { ModalSurface } from "./ModalSurface";
import React from 'react';
import { invoke } from "@tauri-apps/api/core";

interface Props {
  onClose: () => void;
}

export const InstructionsModal: React.FC<Props> = ({ onClose }) => {
  return (
    <div style={{ position: "fixed", top: "50px", left: 0, right: 0, height: "calc(100vh - 50px)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }}>
      <div className="dialog-backdrop" style={{ position: "absolute", top: 0, left: 0, right: 0, bottom: 0, background: "rgba(0,0,0,0.6)", backdropFilter: "blur(calc(4px * var(--backdrop-blur-factor, 1)))", WebkitBackdropFilter: "blur(calc(4px * var(--backdrop-blur-factor, 1)))" }} onClick={onClose} />
      
      <ModalSurface header={<h2 style={{ margin: "0 0 1.5rem 0", color: "#c4b5fd", display: "flex", alignItems: "center", gap: "0.5rem" }}>
          <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>
          Instruções de Uso
        </h2>} onDismiss={onClose} className="modal-content dialog-glass" style={{ background: "var(--tone-1e1b4b, #1e1b4b)", padding: "2rem", borderRadius: "12px", width: "90%", maxWidth: "600px", maxHeight: "calc(100dvh - 100px)", overflowY: "auto", zIndex: 1, border: "1px solid rgba(139, 92, 246, 0.3)", boxShadow: "0 25px 50px -12px rgba(0, 0, 0, 0.7)" }}>
        
        
        <div style={{ display: "flex", flexDirection: "column", gap: "1rem", color: "var(--tone-e2e8f0, #e2e8f0)", lineHeight: "1.6" }}>
          <p style={{ margin: 0 }}>Siga estes passos simples para instalar o mod:</p>
          
          <ol style={{ margin: 0, paddingLeft: "1.5rem", display: "flex", flexDirection: "column", gap: "0.5rem" }}>
            <li><strong>Preparar o backend:</strong> Na Configuração inicial ou em Configurações → Mods → DLSSNR-AMD, escolha RDNA 4 / RX 9000 ou RDNA 3 / RX 7000. Baixe o backend ou importe, em Arquivos locais, a pasta com os pacotes baixados das releases dos mantenedores. O download inclui o instalador, as ferramentas de extração e os componentes do mod; <strong>não inclui a DLL da NVIDIA nem os pesos do modelo</strong>.</li>
            <li><strong>Aguardar a verificação:</strong> Extrair de uma DLL e Usar arquivo existente ficam disponíveis quando o backend estiver pronto. O caminho sublinhado abaixo do download abre a pasta salva. Modelos em cache podem ser reutilizados.</li>
            <li><strong>Fornecer o modelo:</strong> Obtenha a sua <code>nvngx_dlssnr.dll</code> por fora do launcher, em um jogo que a forneça, e escolha Extrair de uma DLL. A versão <strong>310.8.0</strong> gera <code>dlssnr.bin</code> (antigo); a <strong>310.9.16</strong>, com backend compatível, gera também <code>dlssnr-small.bin</code> (novo). São dois arquivos separados. Você também pode usar Usar arquivo existente; para importar o novo, mantenha os dois arquivos na mesma pasta.</li>
            <li><strong>Local dos modelos:</strong> Os arquivos ficam em <code>backend-files/&lt;rdna3|rdna4&gt;/&lt;32|64&gt;/dlssnr-amd/</code>, dentro de <code>~/.config/PeliGames/Mod/DLSSNR/</code>, nas arquiteturas disponíveis. Não mova os modelos para a raiz do mod.</li>
            <li><strong>Instalar no jogo:</strong> Escolha o jogo, a rota e a arquitetura. Use OptiScaler em jogos 64 bits com suporte a upscaling, ou a rota ReShade, Vulkan ou DX9 correspondente à API do jogo. Clique em Instalar.</li>
            <li><strong>Escolher a rede:</strong> No backend 0.0.7 da RX 9000, Rede inicial oferece Novo, Antigo e int4. Instalar ou Reparar define a rede inicial; durante o jogo, alterne no NR Control (End), em ReShade → Add-ons ou com Ctrl+F11. Backends antigos mantêm Novo / Antigo e int4 separado; após alterar suas opções no launcher, clique em Reparar.</li>
            <li><strong>Vetores de movimento:</strong> Nas rotas ReShade, Vulkan e DX9, um backend compatível permite escolher VORT ou LumeniteFX em Opções. VORT já acompanha o backend. Selecionar LumeniteFX e clicar em Instalar ou Reparar baixa os arquivos adicionais do projeto oficial; depois, alterne no menu Add-ons do ReShade. Se LumeniteFX não puder ser usado no jogo, o backend utiliza VORT.</li>
            <li><strong>Atualizar uma instalação anterior:</strong> Não é necessário apagar o backend. Atualize-o e extraia a DLL 310.9.16 para habilitar o novo modelo. Atualizar apenas o PeliGames não fornece essa DLL nem gera novos modelos.</li>
          </ol>

          <div style={{ marginTop: "1rem", padding: "1rem", background: "rgba(0,0,0,0.2)", borderRadius: "8px", border: "1px solid rgba(var(--surface-255-255-255, 255, 255, 255), 0.05)" }}>
            <p style={{ margin: "0 0 0.5rem 0", fontWeight: "bold" }}>Ainda tem dúvidas ou encontrou um problema?</p>
            <button 
              className="btn btn-secondary" 
              onClick={() => invoke('open_url', { url: "https://discord.gg/M5BvWWeQ" })}
              style={{ display: "flex", alignItems: "center", gap: "0.5rem", padding: "0.5rem 1rem", background: "#5865F2", color: "white", border: "none", borderRadius: "6px", cursor: "pointer", fontWeight: "bold" }}
            >
              <svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="currentColor"><path d="M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286zM8.02 15.3312c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9555-2.4189 2.157-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.9555 2.4189-2.1569 2.4189zm7.9748 0c-1.1825 0-2.1569-1.0857-2.1569-2.419 0-1.3332.9554-2.4189 2.1569-2.4189 1.2108 0 2.1757 1.0952 2.1568 2.419 0 1.3332-.946 2.4189-2.1568 2.4189Z"/></svg>
              Entre no nosso Discord
            </button>
          </div>
        </div>

        <div style={{ display: "flex", justifyContent: "flex-end", marginTop: "1.5rem" }}>
          <button className="primary" onClick={onClose} style={{ padding: "0.5rem 1.5rem" }}>
            Entendi
          </button>
        </div>
      </ModalSurface>
    </div>
  );
};
