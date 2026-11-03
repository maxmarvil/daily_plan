export const WEEKDAYS = ["пн", "вт", "ср", "чт", "пт", "сб", "вс"];

export const MONTHS_RU = [
  "январь",
  "февраль",
  "март",
  "апрель",
  "май",
  "июнь",
  "июль",
  "август",
  "сентябрь",
  "октябрь",
  "ноябрь",
  "декабрь",
];

export function pad(n: number): string {
  return String(n).padStart(2, "0");
}

/** Локальная дата в формате 'YYYY-MM-DD' — тот же формат, что в tasks.day */
export function dateStr(d: Date): string {
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

export function todayStr(): string {
  return dateStr(new Date());
}

/** Первая и последняя дата месяца */
export function monthBounds(year: number, month: number): [string, string] {
  return [dateStr(new Date(year, month, 1)), dateStr(new Date(year, month + 1, 0))];
}

export function cap(s: string): string {
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function parse(day: string): Date {
  // полдень, чтобы таймзона не сдвигала дату
  return new Date(`${day}T12:00:00`);
}

export function dayLabel(day: string): string {
  return cap(
    parse(day).toLocaleDateString("ru-RU", {
      weekday: "short",
      day: "numeric",
      month: "long",
    }),
  );
}

export function shortDayLabel(day: string): string {
  return cap(
    parse(day).toLocaleDateString("ru-RU", { day: "numeric", month: "short" }),
  );
}
