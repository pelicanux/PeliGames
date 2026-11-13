import en from "../locales/en.json";
import ptBR from "../locales/pt-BR.json";
import type {AddonMessages} from "../../../_infra/ui-sdk";
export const messages: AddonMessages = {defaultLanguage: "en", resources: {en, "pt-BR": ptBR}};

export type OptiScalerMessageKey = keyof typeof en;
