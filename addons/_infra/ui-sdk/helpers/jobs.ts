import type {AddonHost} from "..";
import type {TransferProgress} from "./downloadProgress";
interface Status<T> {id: number; done: boolean; result?: T; error?: string; progress?: TransferProgress}
function delay(host: AddonHost, milliseconds: number): Promise<void> {
  return new Promise((resolve, reject) => {
    const signal = host.lifecycle.signal;
    const abort = () => {cancel(); signal.removeEventListener("abort", abort); reject(new DOMException("Add-on closed", "AbortError"))};
    const cancel = host.lifecycle.timeout(() => {signal.removeEventListener("abort", abort); resolve()}, milliseconds);
    signal.addEventListener("abort", abort, {once: true});
    if (signal.aborted) abort();
  });
}
/** Bounded polling through the selected service; closing a view cancels the wait. */
export async function waitForAddonJob<T>(host: AddonHost, id: number, onProgress?: (progress: TransferProgress) => void, timeout = 300000): Promise<T | undefined> {
  if (!Number.isSafeInteger(id) || id < 1) throw new Error("Invalid add-on operation ID");
  const deadline = performance.now() + timeout;
  while (!host.lifecycle.signal.aborted) {
    const status = await host.service.call<Status<T>>("job.inspect", {id});
    if (status.id !== id) throw new Error("Add-on operation changed");
    if (host.lifecycle.signal.aborted) break;
    if (status.progress) onProgress?.(status.progress);
    if (status.done) {
      if (status.error) throw new Error(status.error);
      return status.result;
    }
    if (performance.now() >= deadline) throw new Error("Add-on operation timed out");
    await delay(host, 250);
  }
  throw new DOMException("Add-on closed", "AbortError");
}
