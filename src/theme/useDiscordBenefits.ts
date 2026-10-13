import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { verifyDiscordBenefit, type DiscordStatus, type BenefitClaims } from "./discordBenefits";
export function useDiscordBenefits() {
  const [claims, setClaims] = useState<BenefitClaims | null>(null);
  useEffect(() => {
    let active = true;
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
    void refresh();
    const refreshTimer = setInterval(() => void refresh(), 6 * 3600000);
    window.addEventListener("discordConnectionChanged", changed);
    window.addEventListener("focus", focus);
    return () => { active = false; revision++; clearTimeout(expiryTimer); clearInterval(refreshTimer); window.removeEventListener("discordConnectionChanged", changed); window.removeEventListener("focus", focus); };
  }, []);
  return { exclusiveEffects: Boolean(claims && claims.exp > Date.now()), benefitExpiresAt: claims?.exp ?? null };
}
