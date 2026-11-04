import { notifyError } from "../../services/notifications";
import { notifyNexusAccountRequired } from "./requireNexusAccount";

export const nexusDownloadFeedback = "peligames:nexus-download-feedback";
export type DownloadFeedback = { phase: "preparing" | "ready" | "error"; error?: string };
export function showDownloadFeedback(detail: DownloadFeedback) {
  window.dispatchEvent(new CustomEvent<DownloadFeedback>(nexusDownloadFeedback, { detail }));
}

/** Recognize the backend's missing-account errors without relabeling network failures. */
export function notifyNexusDownloadError(error: unknown, language: string) {
  const message = String(error ?? "").replace(/^Error:\s*/, "").trim();
  if (message === "Conecte sua conta na aba Conta Nexus." || message === "Conecte sua conta Nexus.") {
    notifyNexusAccountRequired(language, "download");
  } else {
    notifyError(language === "en" ? "Nexus downloads" : "Downloads Nexus", error);
  }
}
