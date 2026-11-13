import type {AddonLifecycle} from "../../../addons/_infra/ui-sdk";
import {closedError} from "./contracts";

/** Host-owned resources are released synchronously even if custom disposal hangs. */
export class ResourceScope implements AddonLifecycle {
  constructor(private onError: () => void = () => {}) {}
  private controller = new AbortController();
  private resources = new Set<() => void>();
  private callbacks = new Set<() => void | Promise<void>>();
  readonly signal = this.controller.signal;
  assertOpen(): void {if (this.signal.aborted) throw closedError()}
  onDispose(cleanup: () => void | Promise<void>): () => void {
    this.assertOpen(); this.callbacks.add(cleanup); return () => {this.callbacks.delete(cleanup)};
  }
  private track(cleanup: () => void): () => void {
    this.resources.add(cleanup); return () => {if (this.resources.delete(cleanup)) cleanup()};
  }
  timeout(callback: () => void, milliseconds: number): () => void {
    this.assertOpen();
    const timer = setTimeout(() => {cancel(); if (!this.signal.aborted) this.run(callback)}, milliseconds);
    const cancel = this.track(() => clearTimeout(timer)); return cancel;
  }
  interval(callback: () => void, milliseconds: number): () => void {
    this.assertOpen();
    const timer = setInterval(() => {if (!this.signal.aborted) this.run(callback)}, Math.max(16, milliseconds));
    return this.track(() => clearInterval(timer));
  }
  listen(target: EventTarget, type: string, callback: EventListener): () => void {
    this.assertOpen();
    const listener: EventListener = event => this.run(() => callback(event));
    target.addEventListener(type, listener);
    return this.track(() => target.removeEventListener(type, listener));
  }
  private run(callback: () => void): void {
    try {callback()} catch {try {this.onError()} finally {void this.close()}}
  }
  close(): Promise<void> {
    if (this.signal.aborted) return Promise.resolve();
    this.controller.abort();
    for (const release of this.resources) {try {release()} catch {/* Continue releasing other resources. */}}
    this.resources.clear();
    const callbacks = [...this.callbacks]; this.callbacks.clear();
    return settleWithin(callbacks.map(callback => Promise.resolve().then(callback)), 1000);
  }
}
export async function settleWithin(tasks: Promise<unknown>[], milliseconds: number): Promise<void> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {await Promise.race([Promise.allSettled(tasks), new Promise<void>(resolve => {timer = setTimeout(resolve, milliseconds)})])}
  finally {if (timer !== undefined) clearTimeout(timer)}
}
