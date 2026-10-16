import type { ComponentProps } from "react";
import { NexusPopup } from "../modules/nexus/NexusPopup";
import { useI18n } from "../i18n/I18nContext";
export function LaunchOptionsSurface({inline, ...props}:ComponentProps<typeof NexusPopup>&{inline?:boolean}) {
  const {language}=useI18n();
  if(!inline)return <NexusPopup {...props}/>;
  return <div className="modern-inline-editor modern-launch-editor"><div className="modern-inline-head"><strong>{props.title}</strong><button type="button" className="modern-mini-btn" onClick={props.onClose}>{language==="pt"?"Voltar":"Back"}</button></div>{props.children}<div className="modern-inline-actions">{props.footer}</div></div>;
}
