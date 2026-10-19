/** Coalesces edits and serializes writes. flush() also handles leaving a panel. */
export class DebouncedSave {
  private timer: ReturnType<typeof setTimeout> | undefined;
  private pending: (() => Promise<void>) | undefined;
  private active: Promise<void> | undefined;
  constructor(private delay = 450) {}
  schedule(save: () => Promise<void>) {
    this.pending = save;
    if (this.timer) clearTimeout(this.timer);
    this.timer = setTimeout(() => { this.timer = undefined; void this.flush(); }, this.delay);
  }
  async flush(): Promise<void> {
    if (this.timer) { clearTimeout(this.timer); this.timer = undefined; }
    if (this.active) { await this.active; if (this.pending) await this.flush(); return; }
    const save = this.pending;
    this.pending = undefined;
    if (!save) return;
    this.active = save();
    try { await this.active; }
    finally { this.active = undefined; }
    if (this.pending && !this.timer) await this.flush();
  }
}
