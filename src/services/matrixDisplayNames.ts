/** Resolve missing profiles in a bounded batch without delaying the timeline. */
export async function fetchMatrixDisplayNames(users: string[], fetchProfile: (user: string) => Promise<{ displayname?: string }>) {
  const queue = [...new Set(users)];
  const names: Record<string, string> = {};
  let index = 0;
  await Promise.all(Array.from({ length: Math.min(4, queue.length) }, async () => {
    while (index < queue.length) {
      const user = queue[index++];
      try {
        const profile = await fetchProfile(user);
        const name = profile.displayname?.trim();
        if (name) names[user] = [...name].slice(0, 64).join("");
      } catch { /* A missing/temporarily unavailable profile keeps the ID fallback. */ }
    }
  }));
  return names;
}
