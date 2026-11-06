import {useLayoutEffect, useRef, useState} from "react";
export function useLauncherAppearance() {
  const [performanceMode, setPerformanceMode] = useState(() => {try {return localStorage.getItem("performance_mode") === "true";} catch {return false;}});
  const [playGameEntrance, setPlayGameEntrance] = useState(true);
  const [effectsHaveChanged, setEffectsHaveChanged] = useState(false);
  const initialized = useRef(false);
  useLayoutEffect(() => {
    if (initialized.current) setEffectsHaveChanged(true);
    initialized.current = true;
    setPlayGameEntrance(false);
    try {localStorage.setItem("performance_mode", String(performanceMode));} catch { /* Session-only preference. */ }
    document.body.classList.toggle("performance-mode", performanceMode);
  }, [performanceMode]);
  return {performanceMode, setPerformanceMode, playGameEntrance, setPlayGameEntrance, effectsHaveChanged};
}
