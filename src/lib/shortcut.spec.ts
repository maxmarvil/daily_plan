import { describe, it, expect } from "vitest";
import {
  DEFAULT_SHORTCUT,
  comboFromEvent,
  formatCombo,
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

describe("formatCombo", () => {
  it("renders mac symbols", () => {
    expect(formatCombo("Command+Shift+D")).toBe("⌘⇧D");
  });

  it("renders default shortcut", () => {
    expect(formatCombo(DEFAULT_SHORTCUT)).toBe("⌘/⌃⇧D");
  });
});
