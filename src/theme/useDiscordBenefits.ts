import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { verifyDiscordBenefit, type DiscordStatus, type BenefitClaims } from "./discordBenefits";
export function useDiscordBenefits() {
  const [claims, setClaims] = useState<BenefitClaims | null>(null);
  // False until the entitlement saved on this computer has been checked once.
  const [checked, setChecked] = useState(false);
  useEffect(() => {
    let active = true;
    if (import.meta.env.DEV && "__PELI_UI_PREVIEW__" in window) {
      let unsubscribe: (() => void) | undefined;
      void import("../previewDiscordAccount").then(preview => {
        if (!active) return;
        const update = () => { setClaims(preview.previewBenefitClaims()); setChecked(true); };
        unsubscribe = preview.subscribePreviewAccount(update);
        update();
      });
      return () => { active = false; unsubscribe?.(); };
    }
    let revision = 0;
    let lastRefresh = 0;
    let expiryTimer: ReturnType<typeof setTimeout> | undefined;
    const accept = async (status: DiscordStatus) => {
      const current = ++revision;
      const verified = await verifyDiscordBenefit(status);
      if (active && current === revision) {
        clearTimeout(expiryTimer);
        setClaims(verified);
        if (verified) expiryTimer = setTimeout(() => { if (active) setClaims(null); }, Math.max(0, verified.exp - Date.now()));
      }
    };
    const refresh = async () => {
      lastRefresh = Date.now();
      try { await accept(await invoke<DiscordStatus>("load_discord_account")); }
      catch { /* Keep only the already-verified lease until expiry. */ }
    };
    const changed = (event: Event) => { void accept((event as CustomEvent<DiscordStatus>).detail); };
    const focus = () => { if (Date.now() - lastRefresh > 6 * 3600000) void refresh(); };
    // The saved entitlement first (no network), then the online check.
    void invoke<DiscordStatus>("cached_discord_account").then(accept).catch(() => {})
      .finally(() => { if (active) setChecked(true); void refresh(); });
    const refreshTimer = setInterval(() => void refresh(), 6 * 3600000);
    window.addEventListener("discordConnectionChanged", changed);
    window.addEventListener("focus", focus);
    return () => { active = false; revision++; clearTimeout(expiryTimer); clearInterval(refreshTimer); window.removeEventListener("discordConnectionChanged", changed); window.removeEventListener("focus", focus); };
  }, []);
  return { exclusiveEffects: Boolean(claims && claims.exp > Date.now()), benefitExpiresAt: claims?.exp ?? null, benefitsChecked: checked };
}
