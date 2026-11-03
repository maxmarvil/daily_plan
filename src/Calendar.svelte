<script lang="ts">
  import { onMount } from "svelte";
  import Database from "@tauri-apps/plugin-sql";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import {
    WEEKDAYS,
    MONTHS_RU,
    cap,
    dateStr,
    monthBounds,
    dayLabel,
    shortDayLabel,
  } from "./lib/dates";

  type Mode = "year" | "month";

  type CountRow = { day: string; n: number };

  type TaskRow = {
    id: number;
    day: string;
    position: number;
    text: string;
    truncated: number;
    source_id: number | null;
    source_day: string | null;
  };

  let db: Database | null = $state(null);
  let mode: Mode = $state("year");
  let year: number = $state(new Date().getFullYear());
  let month: number = $state(new Date().getMonth());
  let counts: Record<string, number> = $state({});
  let rows: TaskRow[] = $state([]);

  const maxCount = $derived(
    Object.values(counts).reduce((a, b) => Math.max(a, b), 0),
  );

  const title = $derived(
    mode === "year" ? String(year) : `${cap(MONTHS_RU[month])} ${year}`,
  );

  // 12 месячных сеток для режима «Год»
  const monthsGrid = $derived(
    Array.from({ length: 12 }, (_, m) => ({
      m,
      cells: buildMonthCells(year, m),
    })),
  );

  // задачи месяца, сгруппированные по дням (дни в обратном порядке)
  const daysGrouped = $derived.by(() => {
    const map = new Map<string, TaskRow[]>();
    for (const r of rows) {
      const list = map.get(r.day);
      if (list) list.push(r);
      else map.set(r.day, [r]);
    }
    return [...map.entries()];
  });

  function buildMonthCells(y: number, m: number): (string | null)[] {
    const offset = (new Date(y, m, 1).getDay() + 6) % 7; // неделя с понедельника
    const total = new Date(y, m + 1, 0).getDate();
    const cells: (string | null)[] = Array<string | null>(offset).fill(null);
    for (let d = 1; d <= total; d++) cells.push(dateStr(new Date(y, m, d)));
    return cells;
  }

  function heat(day: string): string {
    const n = counts[day] ?? 0;
    if (!n) return "transparent";
    const alpha = Math.round((0.14 + 0.86 * Math.min(1, n / Math.max(1, maxCount))) * 100);
    return `color-mix(in srgb, var(--accent) ${alpha}%, transparent)`;
  }

  async function loadYear() {
    const r = await db!.select<CountRow[]>(
      `SELECT day, COUNT(*) AS n FROM tasks
       WHERE substr(day, 1, 4) = $1
       GROUP BY day`,
      [String(year)],
    );
    counts = Object.fromEntries(r.map((x) => [x.day, x.n]));
  }

  async function loadMonth() {
    const [from, to] = monthBounds(year, month);
    rows = await db!.select<TaskRow[]>(
      `SELECT t.id, t.day, t.position, t.source_id, s.day AS source_day,
              CASE WHEN t.source_id IS NULL THEN t.text
                   ELSE substr(COALESCE(s.text, ''), 1, 40) END AS text,
              CASE WHEN t.source_id IS NOT NULL
                        AND length(COALESCE(s.text, '')) > 40
                   THEN 1 ELSE 0 END AS truncated
       FROM tasks t
       LEFT JOIN tasks s ON s.id = t.source_id
       WHERE t.day BETWEEN $1 AND $2
       ORDER BY t.day DESC, t.position`,
      [from, to],
    );
  }

  $effect(() => {
    if (!db) return;
    // читая state, эффект пересобирает данные при смене режима/периода
    if (mode === "year") void loadYear();
    else void loadMonth();
  });

  function step(dir: number) {
    if (mode === "year") {
      year += dir;
    } else {
      const d = new Date(year, month + dir, 1);
      year = d.getFullYear();
      month = d.getMonth();
    }
  }

  function goToday() {
    const d = new Date();
    year = d.getFullYear();
    month = d.getMonth();
  }

  /** день -> режим «Месяц» на этом месяце */
  function openDay(day: string) {
    const [y, m] = day.split("-").map(Number);
    year = y;
    month = m - 1;
    mode = "month";
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") getCurrentWindow().hide();
  }

  onMount(() => {
    Database.load("sqlite:daily-plan.db").then((loaded) => (db = loaded));
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main class="calendar">
  <header class="head">
    <div class="nav">
      <button class="navbtn" onclick={() => step(-1)} aria-label="Назад">‹</button>
      <div class="period">{title}</div>
      <button class="navbtn" onclick={() => step(1)} aria-label="Вперёд">›</button>
      <button class="today" onclick={goToday}>Сегодня</button>
    </div>
    <div class="seg">
      <button class:active={mode === "month"} onclick={() => (mode = "month")}>
        Месяц
      </button>
      <button class:active={mode === "year"} onclick={() => (mode = "year")}>
        Год
      </button>
    </div>
  </header>

  {#if mode === "year"}
    <section class="year-grid">
      {#each monthsGrid as { m, cells }}
        <div class="month">
          <button class="month-name" onclick={() => openDay(`${year}-${String(m + 1).padStart(2, "0")}-01`)}>
            {cap(MONTHS_RU[m])}
          </button>
          <div class="wd">
            {#each WEEKDAYS as w}<span>{w}</span>{/each}
          </div>
          <div class="cells">
            {#each cells as day}
              {#if day}
                <button
                  class="cell"
                  class:has-tasks={(counts[day] ?? 0) > 0}
                  style:background={heat(day)}
                  onclick={() => openDay(day)}
                  title="{dayLabel(day)}: задач {(counts[day] ?? 0)}"
                >{Number(day.slice(-2))}</button>
              {:else}
                <span class="cell empty"></span>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
    </section>
  {:else}
    <section class="month-list">
      {#each daysGrouped as [d, list]}
        <h3 class="day-head">{dayLabel(d)}</h3>
        {#each list as t}
          <div
            class="task"
            class:transfer={t.source_id !== null}
            role="button"
            tabindex="0"
            onclick={() => t.source_day && openDay(t.source_day)}
            onkeydown={(e) => e.key === "Enter" && t.source_day && openDay(t.source_day)}
          >
            <span class="num">{t.position}.</span>
            <span class="text">{t.text}{t.truncated ? "…" : ""}</span>
            {#if t.source_id !== null && t.source_day}
              <span class="src">↗ {shortDayLabel(t.source_day)}</span>
            {/if}
          </div>
        {/each}
      {/each}
      {#if db && rows.length === 0}
        <p class="none">В этом месяце задач нет</p>
      {/if}
    </section>
  {/if}
</main>

<style>
  .calendar {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family:
      -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif;
    font-size: 14px;
    color: var(--ink);
    background: var(--bg);
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 18px 10px;
    flex-shrink: 0;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .period {
    min-width: 150px;
    text-align: center;
    font-weight: 600;
    font-size: 15px;
  }

  .navbtn {
    border: none;
    background: none;
    color: var(--muted);
    font-size: 20px;
    line-height: 1;
    padding: 4px 8px;
    border-radius: 6px;
    cursor: pointer;
  }

  .navbtn:hover {
    background: var(--hover);
    color: var(--ink);
  }

  .today {
    border: none;
    background: var(--hover);
    color: var(--ink);
    font-size: 12px;
    padding: 5px 10px;
    border-radius: 6px;
    cursor: pointer;
  }

  .seg {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--hover);
  }

  .seg button {
    border: none;
    background: none;
    color: var(--muted);
    font-size: 12px;
    font-weight: 500;
    padding: 4px 12px;
    border-radius: 6px;
    cursor: pointer;
  }

  .seg button.active {
    background: var(--editor-bg);
    color: var(--ink);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }

  /* ——— режим «Год» ——— */

  .year-grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px 20px;
    padding: 10px 18px 18px;
  }

  .month-name {
    border: none;
    background: none;
    padding: 0 0 4px;
    color: var(--ink);
    font-size: 12px;
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }

  .month-name:hover {
    color: var(--accent);
  }

  .wd {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    margin-bottom: 2px;
  }

  .wd span {
    font-size: 9px;
    color: var(--muted);
    text-align: center;
  }

  .cells {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 2px;
  }

  .cell {
    aspect-ratio: 1;
    border: none;
    border-radius: 4px;
    background: transparent;
    color: var(--ink);
    font-size: 10px;
    font-variant-numeric: tabular-nums;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
  }

  .cell:hover {
    outline: 1.5px solid var(--accent);
    outline-offset: -1.5px;
  }

  .cell.empty {
    cursor: default;
  }

  .cell.has-tasks {
    font-weight: 600;
  }

  /* ——— режим «Месяц» ——— */

  .month-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 18px 18px;
  }

  .day-head {
    margin: 14px 0 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .task {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 8px;
    cursor: pointer;
  }

  .task:hover {
    background: var(--hover);
  }

  .task.transfer {
    background: color-mix(in srgb, var(--accent) 16%, var(--bg));
  }

  .num {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .text {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .src {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 11px;
    color: var(--accent);
    white-space: nowrap;
  }

  .none {
    margin-top: 40px;
    text-align: center;
    color: var(--muted);
  }
</style>
