export const DEFAULT_SHORTCUT = "CmdOrCtrl+Shift+D";
export const DEFAULT_NEW_TASK_SHORTCUT = "CmdOrCtrl+Shift+N";

/** Отображение канонических модификаторов. */
const MOD_SYMBOLS: Record<string, string> = {
  Command: "⌘",
  Control: "⌃",
  Alt: "⌥",
  Shift: "⇧",
};

/** Алиасы модификаторов: сюда приходят и строки из event-рекордера, и `Shortcut::to_string()` из Rust. */
const MODIFIER_ALIASES: Record<string, string> = {
  command: "Command",
  cmd: "Command",
  super: "Command",
  meta: "Command",
  control: "Control",
  ctrl: "Control",
  alt: "Alt",
  option: "Alt",
  shift: "Shift",
};

/** `CmdOrCtrl` раскрывается в конкретную платформу при нормализации. */
const CMD_OR_CTRL_ALIASES = new Set([
  "commandorcontrol",
  "commandorctrl",
  "cmdorctrl",
  "cmdorcontrol",
]);

/** Порядок модификаторов при отображении и сравнении. */
const MODIFIER_ORDER = ["Command", "Control", "Alt", "Shift"];

/** Имена клавиш из `Code::to_string()` (keyboard-types) и event-рекордера → общий вид. */
const KEY_ALIASES: Record<string, string> = {
  arrowup: "Up",
  up: "Up",
  arrowdown: "Down",
  down: "Down",
  arrowleft: "Left",
  left: "Left",
  arrowright: "Right",
  right: "Right",
  space: "Space",
  enter: "Enter",
  numpadenter: "Enter",
  tab: "Tab",
  escape: "Escape",
  backspace: "Backspace",
  delete: "Delete",
  home: "Home",
  end: "End",
  pageup: "PageUp",
  pagedown: "PageDown",
  comma: ",",
  period: ".",
  slash: "/",
  minus: "-",
  equal: "=",
  semicolon: ";",
  quote: "'",
  bracketleft: "[",
  bracketright: "]",
  backslash: "\\",
  backquote: "`",
};

function isMacPlatform(): boolean {
  try {
    return /mac/i.test(navigator.platform);
  } catch {
    return false;
  }
}

function normalizeKeyToken(token: string): string {
  const upper = token.toUpperCase();
  if (/^KEY[A-Z]$/.test(upper)) return upper.slice(3);
  if (/^DIGIT[0-9]$/.test(upper)) return upper.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(upper)) return upper;
  return KEY_ALIASES[token.toLowerCase()] ?? token;
}

/**
 * Приводит комбинацию к каноническому виду: модификаторы в фиксированном
 * порядке, `CmdOrCtrl` → `Command`/`Control` по платформе, клавиши — как в
 * рекордере (`KeyD` → `D`, `ArrowUp` → `Up`).
 *
 * Нужна потому, что Rust отдаёт `Shortcut::to_string()`
 * (`shift+super+KeyD`), а UI и настройки работают с `Command+Shift+D`.
 */
export function normalizeComboTokens(
  combo: string,
  isMac: boolean = isMacPlatform(),
): string[] {
  const mods = new Set<string>();
  let key = "";

  for (const raw of combo.split("+")) {
    const token = raw.trim();
    if (!token) continue;
    const lower = token.toLowerCase();
    if (CMD_OR_CTRL_ALIASES.has(lower)) {
      mods.add(isMac ? "Command" : "Control");
      continue;
    }
    const modifier = MODIFIER_ALIASES[lower];
    if (modifier) {
      mods.add(modifier);
      continue;
    }
    key = normalizeKeyToken(token);
  }

  const ordered = MODIFIER_ORDER.filter((modifier) => mods.has(modifier));
  return key ? [...ordered, key] : ordered;
}

/** 'shift+super+KeyD' → 'Command+Shift+D' (macOS). */
export function normalizeCombo(
  combo: string,
  isMac: boolean = isMacPlatform(),
): string {
  return normalizeComboTokens(combo, isMac).join("+");
}

/** Сравнение комбинаций без учёта платформенных алиасов Rust/JS. */
export function combosEqual(
  a: string,
  b: string,
  isMac: boolean = isMacPlatform(),
): boolean {
  return normalizeCombo(a, isMac) === normalizeCombo(b, isMac);
}

/** 'Command+Shift+D' → ['⌘', '⇧', 'D'] для отображения. */
export function formatComboParts(
  combo: string,
  isMac: boolean = isMacPlatform(),
): string[] {
  return normalizeComboTokens(combo, isMac).map(
    (token) => MOD_SYMBOLS[token] ?? token,
  );
}

/** 'Command+Shift+D' → '⌘ ⇧ D' для статусов и простых подписей. */
export function formatCombo(
  combo: string,
  isMac: boolean = isMacPlatform(),
): string {
  return formatComboParts(combo, isMac).join(" ");
}

/**
 * Собирает каноническую строку ('Command+Shift+D') из события клавиатуры.
 * Использует `e.code`, чтобы не зависеть от текущей языковой раскладки.
 * Требует хотя бы один модификатор (как на macOS).
 */
export function comboFromEvent(e: KeyboardEvent): string | null {
  if (e.repeat) return null;

  // только модификатор — не комбинация
  if (["Meta", "Control", "Shift", "Alt", "AltGraph"].includes(e.key)) {
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
