import { parseBenchmark } from "./mangoHudBenchmarks";
self.onmessage = (event: MessageEvent<(string|null)[]>) => {
  try { self.postMessage({ results: event.data.map(csv=>csv===null?null:parseBenchmark(csv)) }); }
  catch(error) { self.postMessage({ error: error instanceof Error ? error.message : String(error) }); }
};
