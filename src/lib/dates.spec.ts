import { describe, it, expect } from "vitest";
import { cap, dateStr, monthBounds, todayStr } from "./dates";

describe("dateStr", () => {
  it("formats local date as YYYY-MM-DD using noon", () => {
    expect(dateStr(new Date(2026, 10, 3))).toBe("2026-11-03");
    expect(dateStr(new Date(2026, 0, 31))).toBe("2026-01-31");
  });
});

describe("monthBounds", () => {
  it("returns first and last day of month", () => {
    expect(monthBounds(2026, 10)).toEqual(["2026-11-01", "2026-11-30"]);
    expect(monthBounds(2024, 1)).toEqual(["2024-02-01", "2024-02-29"]);
  });
});

describe("cap", () => {
  it("capitalizes first letter", () => {
    expect(cap("ноябрь")).toBe("Ноябрь");
    expect(cap("")).toBe("");
  });
});

describe("todayStr", () => {
  it("returns today in YYYY-MM-DD", () => {
    const s = todayStr();
    expect(/^\d{4}-\d{2}-\d{2}$/.test(s)).toBe(true);
  });
});
