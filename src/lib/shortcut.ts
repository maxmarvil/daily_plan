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

/** 'Command+Shift+D' -> '⌘⇧D' для отображения */
export function formatCombo(combo: string): string {
  return combo
    .split("+")
    .map((part) => MOD_SYMBOLS[part] ?? part)
    .join("");
}

/**
 * Собирает каноническую строку ('Command+Shift+D') из события клавиатуры.
 * Использует `e.code`, чтобы не зависеть от текущей языковой раскладки.
 * Требует хотя бы один модификатор (как на macOS).
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (e.repeat) return null;

  // только модификатор — не комбинация
  if (
    ["Meta", "Control", "Shift", "Alt", "AltGraph"].includes(e.key)
  ) {
    return null;
  }

  let key: string | null = null;

  if (/^Key[A-Z]$/.test(e.code)) {
    key = e.code.slice(3);
  } else if (/^Digit[0-9]$/.test(e.code)) {
    key = e.code.slice(5);
  } else if (/^F([1-9]|1[0-9]|2[0-4])$/.test(e.key)) {
    key = e.key;
  } else {
    // специальные клавиши: Space, Enter, Arrow*, Tab, Escape и т.д.
    // code обычно соответствует именам, которые принимает global-hotkey
    switch (e.code) {
      case "Space":
        key = "Space";
        break;
      case "Enter":
      case "NumpadEnter":
        key = "Enter";
        break;
      case "Tab":
        key = "Tab";
        break;
      case "Escape":
        key = "Escape";
        break;
      case "Backspace":
        key = "Backspace";
        break;
      case "Delete":
        key = "Delete";
        break;
      case "ArrowUp":
        key = "Up";
        break;
      case "ArrowDown":
        key = "Down";
        break;
      case "ArrowLeft":
        key = "Left";
        break;
      case "ArrowRight":
        key = "Right";
        break;
      case "Home":
        key = "Home";
        break;
      case "End":
        key = "End";
        break;
      case "PageUp":
        key = "PageUp";
        break;
      case "PageDown":
        key = "PageDown";
        break;
      case "Comma":
        key = ",";
        break;
      case "Period":
        key = ".";
        break;
      case "Slash":
        key = "/";
        break;
      case "Minus":
        key = "-";
        break;
      case "Equal":
        key = "=";
        break;
      case "Semicolon":
        key = ";";
        break;
      case "Quote":
        key = "'";
        break;
      case "BracketLeft":
        key = "[";
        break;
      case "BracketRight":
        key = "]";
        break;
      case "Backslash":
        key = "\\";
        break;
      case "Backquote":
        key = "`";
        break;
      default:
        // буквы нелатинских раскладок уже обработаны выше через Key[A-Z]
        key = null;
    }
  }

  if (!key) return null;

  const mods: string[] = [];
  if (e.ctrlKey) mods.push("Control");
  if (e.altKey) mods.push("Alt");
  if (e.metaKey) mods.push("Command");
  if (e.shiftKey) mods.push("Shift");
  if (mods.length === 0) return null;

  return [...mods, key].join("+");
}
