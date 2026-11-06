import {translator} from "../i18n/engine";
import ptInstallation from "../i18n/locales/pt-BR/installation.json";

export function isMissingModel(error: unknown): boolean {
  const text = String(error);
  return text.includes("MODEL_BIN_NOT_FOUND") || /the package has no model file:/i.test(text) || /Failed to copy bin file:.*(?:No such file or directory|os error 2)/i.test(text);
}

export function localizeInstallationMessage(message: string, language: string): string {
  const t = translator(language);
  const pt = ptInstallation;
  const replacements: [string, string][] = [
    ["MODEL_BIN_NOT_FOUND", t("installation", "missingModel")],
    [pt.incompatibleTitle, t("installation", "incompatibleTitle")],
    [pt.incompatibleBody, t("installation", "incompatibleBody")],
    ["Arquivos do backend não encontrados! Clique no botão de Atualizar no topo direito para baixar os arquivos necessários.", t("installation", "backendMissing")],
    ["Log completo:", t("installation", "fullLog")],
    ["Failed to copy bin file:", t("installation", "copyBin")],
    ["Failed to create temporary directory:", t("installation", "tempDirectory")],
    ["Failed to extract bundled resources:", t("installation", "extractResources")],
    ["Failed to execute cp command for extraction", t("installation", "extractCommand")],
    ["Failed to execute bash process:", t("installation", "bashProcess")],
    ["Script Error (code:", t("installation", "scriptError")],
    ["Success:", t("installation", "success")],
  ];
  return replacements.reduce((text, [source, translated]) => text.split(source).join(translated), message);
}
