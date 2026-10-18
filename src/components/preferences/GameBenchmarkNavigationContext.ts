import { createContext } from "react";

/** Open benchmarks inside the current game-details panel. */
export const GameBenchmarkNavigationContext = createContext<(() => void) | null>(null);
