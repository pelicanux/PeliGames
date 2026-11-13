import type {AddonBridge} from "./bridge";
import {closedError, selectionKey, type SelectionSnapshot} from "./contracts";

interface Entry {references: number; closing?: Promise<void>}
export interface ServiceLease {call<T>(method: string, params: unknown): Promise<T>; release(): Promise<void>}
/** Multiple views share a worker; only the last view closes that selection. */
export class ServiceLeases {
  private entries = new Map<string, Entry>();
  constructor(private bridge: AddonBridge) {}
  async acquire(id: string, selection: SelectionSnapshot, signal: AbortSignal): Promise<ServiceLease> {
    const key = selectionKey(id, selection);
    let entry = this.entries.get(key);
    while (entry?.closing) {await entry.closing; entry = this.entries.get(key)}
    if (signal.aborted) throw closedError();
    if (!entry) {entry = {references: 0}; this.entries.set(key, entry)}
    const owned = entry;
    owned.references++;
    let released = false;
    let closing: Promise<void> | undefined;
    return {
      call: async <T,>(method: string, params: unknown) => {
        if (released || signal.aborted) throw closedError();
        const result = await this.bridge.call<T>(id, selection, method, params);
        if (released || signal.aborted) throw closedError();
        return result;
      },
      release: () => {
        if (released) return closing ?? Promise.resolve();
        released = true;
        if (--owned.references !== 0) return Promise.resolve();
        closing = this.bridge.stop(id, selection).catch(() => {/* Native status retains service failures. */}).finally(() => {
          if (this.entries.get(key) === owned) this.entries.delete(key);
        });
        owned.closing = closing;
        return closing;
      },
    };
  }
}
