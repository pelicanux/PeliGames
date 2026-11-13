import type {ComponentProps} from "react";
import {MenuIcon} from "./MenuIcon";
import "./componentCatalog.css";

export interface ComponentCard<T extends string> {
  id: T; label: string; description: string;
  icon: ComponentProps<typeof MenuIcon>["name"];
  disabled?: boolean; buttonId?: string;
}
export function ComponentCatalog<T extends string>({label, cards, disabled=false, onOpen}: {
  label: string; cards: readonly ComponentCard<T>[]; disabled?: boolean; onOpen: (id: T) => void;
}) {
  return <nav className="component-catalog addon-catalog mod-workspace" aria-label={label}>
    {cards.map(card => <button type="button" className="component-catalog-button addon-catalog-button" key={card.id} id={card.buttonId}
      aria-label={card.label} disabled={disabled || card.disabled} onClick={() => onOpen(card.id)}>
      <span className="mod-component-icon"><MenuIcon name={card.icon}/></span>
      <span className="addon-catalog-copy"><strong>{card.label}</strong><small>{card.description}</small></span>
      <span className="addon-catalog-arrow" aria-hidden="true">›</span>
    </button>)}
  </nav>;
}
