import { describe, it, expect } from "vitest";
import {
  DEFAULT_SHORTCUT,
  comboFromEvent,
  combosEqual,
  formatCombo,
  formatComboParts,
  normalizeCombo,
} from "./shortcut";

function key(
  code: string,
  keyValue: string,
  mods: {
    ctrl?: boolean;
    alt?: boolean;
    meta?: boolean;
    shift?: boolean;
    repeat?: boolean;
  } = {},
) {
  return new KeyboardEvent("keydown", {
    code,
    key: keyValue,
    ctrlKey: mods.ctrl,
    altKey: mods.alt,
    metaKey: mods.meta,
    shiftKey: mods.shift,
    repeat: mods.repeat,
  });
}

describe("comboFromEvent", () => {
  it("returns null for bare letter", () => {
    expect(comboFromEvent(key("KeyA", "a"))).toBeNull();
  });

  it("parses Command+Shift+D", () => {
    expect(comboFromEvent(key("KeyD", "D", { meta: true, shift: true }))).toBe(
      "Command+Shift+D",
    );
  });

  it("is layout-independent (cyrillic Д yields KeyD code)", () => {
    expect(comboFromEvent(key("KeyD", "Д", { meta: true, shift: true }))).toBe(
      "Command+Shift+D",
    );
  });

  it("parses digit with modifiers", () => {
    expect(
      comboFromEvent(key("Digit5", "5", { ctrl: true, alt: true })),
    ).toBe("Control+Alt+5");
  });

  it("ignores repeat events", () => {
    expect(
      comboFromEvent(key("KeyD", "D", { meta: true, repeat: true })),
    ).toBeNull();
  });

  it("ignores lone modifier keys", () => {
    expect(
      comboFromEvent(key("ShiftLeft", "Shift", { shift: true })),
    ).toBeNull();
  });

  it("maps special keys by code", () => {
    expect(comboFromEvent(key("Space", " ", { meta: true }))).toBe(
      "Command+Space",
    );
    expect(comboFromEvent(key("Enter", "Enter", { ctrl: true }))).toBe(
      "Control+Enter",
    );
  });
});

describe("normalizeCombo", () => {
  it("normalizes the Rust display form to the UI form", () => {
    // Shortcut::to_string() из global-hotkey на macOS
    expect(normalizeCombo("shift+super+KeyD", true)).toBe("Command+Shift+D");
    expect(normalizeCombo("shift+alt+KeyN", true)).toBe("Alt+Shift+N");
  });

  it("expands CmdOrCtrl per platform", () => {
    expect(normalizeCombo(DEFAULT_SHORTCUT, true)).toBe("Command+Shift+D");
    expect(normalizeCombo(DEFAULT_SHORTCUT, false)).toBe("Control+Shift+D");
  });

  it("keeps modifier order stable", () => {
    expect(normalizeCombo("Shift+Alt+Command+D", true)).toBe(
      "Command+Alt+Shift+D",
    );
  });

  it("maps arrow and punctuation aliases", () => {
    expect(normalizeCombo("Command+ArrowUp", true)).toBe("Command+Up");
    expect(normalizeCombo("Command+Up", true)).toBe("Command+Up");
    expect(normalizeCombo("Command+Comma", true)).toBe("Command+,");
  });
});

describe("combosEqual", () => {
  it("treats Rust and UI forms as equal", () => {
    expect(combosEqual("shift+super+KeyD", DEFAULT_SHORTCUT, true)).toBe(true);
  });

  it("detects different combinations", () => {
    expect(combosEqual("shift+super+KeyD", "shift+super+KeyN", true)).toBe(false);
  });
});

describe("formatCombo", () => {
  it("renders mac symbols separated", () => {
    expect(formatCombo("Command+Shift+D", true)).toBe("⌘ ⇧ D");
  });

  it("renders default shortcut for mac", () => {
    expect(formatCombo(DEFAULT_SHORTCUT, true)).toBe("⌘ ⇧ D");
  });

  it("renders Rust display form", () => {
    expect(formatCombo("shift+super+KeyD", true)).toBe("⌘ ⇧ D");
  });
});

describe("formatComboParts", () => {
  it("returns separate chips", () => {
    expect(formatComboParts("Command+Shift+D", true)).toEqual(["⌘", "⇧", "D"]);
    expect(formatComboParts("shift+alt+KeyN", true)).toEqual(["⌥", "⇧", "N"]);
  });
});
