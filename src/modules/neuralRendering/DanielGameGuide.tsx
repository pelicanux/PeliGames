import { useI18n } from "../../i18n/I18nContext";

export function DanielGameGuide({ version }: { version: string }) {
  const pt = useI18n().language === "pt";
  const keys = version === "0.8.0"
    ? [["End", pt ? "Abrir / fechar o painel Daniel" : "Open / close the Daniel panel"], ["PgUp / PgDn", pt ? "Trocar de aba" : "Switch tabs"], ["↑ / ↓", pt ? "Selecionar uma opção" : "Select an option"], ["← / →", pt ? "Alterar o valor" : "Change the value"], ["Backspace", pt ? "Restaurar o padrão da opção" : "Restore the option default"], ["Page Down", pt ? "Menu AMDNR / OptiScaler (com o painel Daniel fechado)" : "AMDNR / OptiScaler menu (with the Daniel panel closed)"]]
    : [["Insert", pt ? "Abrir AMDNR / OptiScaler → Neural" : "Open AMDNR / OptiScaler → Neural"]];
  return <>
    <p className="modern-dlss-description">{pt ? "Abra o jogo com Proton e ative FSR, DLSS ou XeSS nas opções gráficas. O motor precisa de um upscaler compatível para processar a imagem." : "Launch with Proton and enable FSR, DLSS or XeSS in graphics settings. The runtime needs a compatible upscaler to process the image."}</p>
    <dl className="daniel-nr-shortcuts">{keys.map(([key, text]) => <div key={key}><dt><kbd>{key}</kbd></dt><dd>{text}</dd></div>)}</dl>
    {version === "0.8.0" && <p className="modern-dlss-description daniel-nr-note">{pt ? "O painel Daniel 0.8 é operado pelo teclado. Os ajustes de imagem ficam dentro do jogo." : "The Daniel 0.8 panel uses keyboard controls. Adjust the image inside the game."}</p>}
  </>;
}
