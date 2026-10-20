/** Refresh even when one of several selected components fails after others installed. */
export async function refreshWineCatalogAfter<T>(operation: () => Promise<T>, refresh: () => Promise<unknown>): Promise<T> {
  let failed = false, failure: unknown;
  let result: T | undefined;
  try { result = await operation(); }
  catch (error) { failed = true; failure = error; }
  try { await refresh(); }
  catch (error) { if (!failed) throw error; }
  if (failed) throw failure;
  return result as T;
}
