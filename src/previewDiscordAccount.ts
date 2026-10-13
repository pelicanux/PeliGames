// Visual fixtures only: imported dynamically by the development preview.
import type { BenefitClaims, DiscordStatus } from "./theme/discordBenefits";
export type PreviewAccount = "owner" | "free";
let account: PreviewAccount = new URLSearchParams(location.search).get("account") === "owner" ? "owner" : "free";
const issuedAt = Date.now();
const expiresAt = issuedAt + 7 * 86400000;
const listeners = new Set<() => void>();
export function previewAccount() { return account; }
export function previewDiscordStatus(): DiscordStatus {
  return { configured: true, pending: false, offline: false, account: account === "owner"
    ? { id: "371769979558821902", name: "Pelicanux · conta simulada", in_server: true, benefit: "owner", valid_until: expiresAt }
    : null };
}
export function previewBenefitClaims(): BenefitClaims | null {
  return account === "owner" ? { sub: "371769979558821902", sid: "preview-only", aud: "peligames-cover-effects", guild: "1088170837921779812", benefit: "owner", iat: issuedAt, exp: expiresAt } : null;
}
export function subscribePreviewAccount(listener: () => void) { listeners.add(listener); return () => { listeners.delete(listener); }; }
export function setPreviewAccount(next: PreviewAccount) {
  account = next;
  const url = new URL(location.href); url.searchParams.set("account", next); history.replaceState(null, "", url);
  for (const listener of listeners) listener();
  window.dispatchEvent(new CustomEvent<DiscordStatus>("discordConnectionChanged", { detail: previewDiscordStatus() }));
}
