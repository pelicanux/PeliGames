export const nexusDownloadFeedback = "peligames:nexus-download-feedback";
export type DownloadFeedback = { phase: "preparing" | "ready" | "error"; error?: string };
export function showDownloadFeedback(detail: DownloadFeedback) {
  window.dispatchEvent(new CustomEvent<DownloadFeedback>(nexusDownloadFeedback, { detail }));
}
