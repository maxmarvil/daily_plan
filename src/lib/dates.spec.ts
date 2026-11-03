import { describe, it, expect } from "vitest";
import {
  buildMonthCells,
  cap,
  dateStr,
  monthBounds,
  taskCountWord,
  todayStr,
  yearBounds,
} from "./dates";

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

describe("yearBounds", () => {
  it("returns first and last day of year", () => {
    expect(yearBounds(2026)).toEqual(["2026-01-01", "2026-12-31"]);
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

describe("taskCountWord", () => {
  it("declines by Russian rules", () => {
    expect(taskCountWord(1)).toBe("задача");
    expect(taskCountWord(2)).toBe("задачи");
    expect(taskCountWord(4)).toBe("задачи");
    expect(taskCountWord(5)).toBe("задач");
    expect(taskCountWord(11)).toBe("задач");
    expect(taskCountWord(12)).toBe("задач");
    expect(taskCountWord(14)).toBe("задач");
    expect(taskCountWord(21)).toBe("задача");
    expect(taskCountWord(22)).toBe("задачи");
    expect(taskCountWord(25)).toBe("задач");
  });
});

describe("buildMonthCells", () => {
  it("pads the first week up to Monday", () => {
    // 1 ноября 2026 — воскресенье, значит 6 пустых ячеек (пн..сб)
    const cells = buildMonthCells(2026, 10);
    expect(cells.slice(0, 6)).toEqual([null, null, null, null, null, null]);
    expect(cells[6]).toBe("2026-11-01");
    expect(cells.at(-1)).toBe("2026-11-30");
    expect(cells.filter(Boolean)).toHaveLength(30);
  });

  it("handles leap February", () => {
    // 1 февраля 2024 — четверг: 3 пустые ячейки (пн, вт, ср)
    const cells = buildMonthCells(2024, 1);
    expect(cells.slice(0, 3)).toEqual([null, null, null]);
    expect(cells[3]).toBe("2024-02-01");
    expect(cells.filter(Boolean)).toHaveLength(29);
    expect(cells).toContain("2024-02-29");
  });
});
