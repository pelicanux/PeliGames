import { createContext } from "react";

/** Mount catalog navigation in the fixed game header, outside its scrolling body. */
export const CatalogNavigationContext = createContext<HTMLElement | null>(null);
