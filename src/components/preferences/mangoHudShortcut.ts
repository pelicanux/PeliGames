const names: Record<string, string> = {
  Enter: "Return", NumpadEnter: "KP_Enter", Space: "space", Backspace: "BackSpace",
  Tab: "Tab", Insert: "Insert", Delete: "Delete", Home: "Home", End: "End",
  PageUp: "Page_Up", PageDown: "Page_Down", ArrowUp: "Up", ArrowDown: "Down",
  ArrowLeft: "Left", ArrowRight: "Right", Pause: "Pause", CapsLock: "Caps_Lock",
  ScrollLock: "Scroll_Lock", PrintScreen: "Print", Minus: "minus", Equal: "equal",
  BracketLeft: "bracketleft", BracketRight: "bracketright", Backslash: "backslash",
  Semicolon: "semicolon", Quote: "apostrophe", Backquote: "grave", Comma: "comma",
  Period: "period", Slash: "slash", NumpadAdd: "KP_Add", NumpadSubtract: "KP_Subtract",
  NumpadMultiply: "KP_Multiply", NumpadDivide: "KP_Divide", NumpadDecimal: "KP_Decimal",
};
export function mangoHudShortcut(event: Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">, held: Set<string>): string | null {
  const code = event.code;
  const key = /^Key[A-Z]$/.test(code) ? code.slice(3).toLowerCase()
    : /^Digit[0-9]$/.test(code) ? code.slice(5)
    : /^Numpad[0-9]$/.test(code) ? `KP_${code.slice(6)}`
    : /^F([1-9]|1[0-9]|2[0-4])$/.test(code) ? code : names[code];
  if (!key) return null;
  const modifiers: string[] = [];
  for (const [active, prefix, name] of [
    [event.ctrlKey, "Control", "Control"], [event.altKey, "Alt", "Alt"],
    [event.shiftKey, "Shift", "Shift"], [event.metaKey, "Meta", "Super"],
  ] as const) {
    if (active) modifiers.push(`${name}_${held.has(`${prefix}Right`) ? "R" : "L"}`);
  }
  return [...modifiers, key].join("+");
}

export const shortcutLabel = (value: string) => value.split("+").map(key => ({
  Shift_R: "Shift Direito", Shift_L: "Shift Esquerdo", Control_R: "Ctrl Direito", Control_L: "Ctrl Esquerdo",
  Alt_R: "Alt Direito", Alt_L: "Alt Esquerdo", Super_R: "Super Direito", Super_L: "Super Esquerdo",
  space: "Espaço", Return: "Enter", BackSpace: "Backspace", Page_Up: "Page Up", Page_Down: "Page Down",
}[key] ?? (key.length === 1 ? key.toUpperCase() : key))).join(" + ");
