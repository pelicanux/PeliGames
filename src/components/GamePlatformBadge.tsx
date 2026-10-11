/** Platform resolved by the Nexus module, rather than inferred from the store. */
export function GamePlatformBadge({ platform }: { platform?: "native" | "proton" }) {
  if (!platform) return null;
  const label = platform === "native" ? "Linux" : "Windows · Proton / Wine";
  return <span className="game-platform-badge" role="img" aria-label={label} title={label}>
    {platform === "proton" ? <svg viewBox="0 0 24 24" aria-hidden="true" fill="currentColor">
      <path d="M2 4.4 10.5 3.2v8.1H2zm10-1.4L22 1.5v9.8H12zM2 12.7h8.5v8.1L2 19.6zm10 0h10v9.8L12 21z" />
    </svg> : <span className="game-platform-tux" aria-hidden="true" />}
  </span>;
}
