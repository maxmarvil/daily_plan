export const DEFAULT_SHORTCUT = "CmdOrCtrl+Shift+D";

const MOD_SYMBOLS: Record<string, string> = {
  Command: "⌘",
  Cmd: "⌘",
  Super: "⌘",
  Control: "⌃",
  Ctrl: "⌃",
  Alt: "⌥",
  Option: "⌥",
  Shift: "⇧",
  CommandOrControl: "⌘/⌃",
  CmdOrCtrl: "⌘/⌃",
};

const KEY_NAMES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

/** 'Command+Shift+D' -> '⌘⇧D' для отображения */
export function formatCombo(combo: string): string {
  return combo
    .split("+")
    .map((part) => MOD_SYMBOLS[part] ?? KEY_NAMES[part] ?? part)
    .join("");
}

/**
 * Собирает каноническую строку ('Command+Shift+D') из события клавиатуры.
 * Требует хотя бы один модификатор (как на macOS). Только модификаторы -> null.
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (e.repeat) return null;

  const modifiers = ["Meta", "Control", "Shift", "Alt", "AltGraph"];
  if (modifiers.includes(e.key)) return null;

  let key: string | null;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.key)) key = e.key;
  else if (KEY_NAMES[e.key]) key = KEY_NAMES[e.key];
  else if (e.key.length === 1) key = e.key.toUpperCase();
  else key = null;
  if (!key) return null;

  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.metaKey) mods.push("Command");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;

  return [...mods, key].join("+");
}
