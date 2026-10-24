/** Keep quiet music visible without amplifying silence or flattening each band. */
export class NeonWaveAudioResponse {
  private peak = .15;
  private last = 0;

  update(values: number[], now: number): number[] {
    const clean = values.map(value => Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0);
    const dt = this.last ? Math.min(1, Math.max(0, (now - this.last) / 1000)) : 0;
    this.last = now;
    // Follow louder passages immediately, quieter passages gradually.
    this.peak = Math.max(.15, ...clean, this.peak * Math.exp(-dt / 2.5));
    return clean.map(value => value <= .002 ? 0 : Math.pow(Math.min(1, value / this.peak), .75));
  }
}
