<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getDb } from "./lib/db";
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
    comments: number;
  };

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let mode: Mode = $state("year");
  let year: number = $state(new Date().getFullYear());
  let month: number = $state(new Date().getMonth());
  let counts: Record<string, number> = $state({});
  let rows: TaskRow[] = $state([]);
  let transferring: TaskRow | null = $state(null);

  const maxCount = $derived(
    Object.values(counts).reduce((a, b) => Math.max(a, b), 0),
  );

  const title = $derived(
    mode === "year" ? String(year) : `${cap(MONTHS_RU[month])} ${year}`,
  );

  const monthsGrid = $derived(
    Array.from({ length: 12 }, (_, m) => ({
      m,
      cells: buildMonthCells(year, m),
    })),
  );

  const daysGrouped = $derived.by(() => {
    const map = new Map<string, TaskRow[]>();
    for (const r of rows) {
      const list = map.get(r.day);
      if (list) list.push(r);
      else map.set(r.day, [r]);
    }
    return [...map.entries()];
  });

  // защита от stale-ответов при быстром переключении режимов/месяцев
  let loadTicket = 0;

  function buildMonthCells(y: number, m: number): (string | null)[] {
    const offset = (new Date(y, m, 1).getDay() + 6) % 7;
    const total = new Date(y, m + 1, 0).getDate();
    const cells: (string | null)[] = Array<string | null>(offset).fill(null);
    for (let d = 1; d <= total; d++) cells.push(dateStr(new Date(y, m, d)));
    return cells;
  }

  function heat(day: string): string {
    const n = counts[day] ?? 0;
    if (!n) return "transparent";
    const alpha = Math.round(
      (0.14 + 0.86 * Math.min(1, n / Math.max(1, maxCount))) * 100,
    );
    return `color-mix(in srgb, var(--accent) ${alpha}%, transparent)`;
  }

  async function loadYear() {
    if (!db) return;
    const myYear = year;
    const ticket = ++loadTicket;
    const r = await db.select<CountRow[]>(
      `SELECT day, COUNT(*) AS n FROM tasks
       WHERE substr(day, 1, 4) = $1
       GROUP BY day`,
      [String(myYear)],
    );
    if (ticket !== loadTicket || mode !== "year" || year !== myYear) return;
    counts = Object.fromEntries(r.map((x) => [x.day, x.n]));
  }

  async function loadMonth() {
    if (!db) return;
    const [myYear, myMonth] = [year, month];
    const ticket = ++loadTicket;
    const [from, to] = monthBounds(myYear, myMonth);
    const result = await db.select<TaskRow[]>(
      `SELECT t.id, t.day, t.position, t.source_id, s.day AS source_day,
              CASE WHEN t.source_id IS NULL THEN t.text
                   ELSE substr(COALESCE(s.text, ''), 1, 40) END AS text,
              CASE WHEN t.source_id IS NOT NULL
                        AND length(COALESCE(s.text, '')) > 40
                   THEN 1 ELSE 0 END AS truncated,
              (SELECT COUNT(*) FROM comments c
                WHERE c.task_id = COALESCE(t.source_id, t.id)) AS comments
       FROM tasks t
       LEFT JOIN tasks s ON s.id = t.source_id
       WHERE t.day BETWEEN $1 AND $2
       ORDER BY t.day DESC, t.position`,
      [from, to],
    );
    if (
      ticket !== loadTicket ||
      mode !== "month" ||
      year !== myYear ||
      month !== myMonth
    ) {
      return;
    }
    rows = result;
  }

  $effect(() => {
    if (!db) return;
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

  function openDay(day: string) {
    const [y, m] = day.split("-").map(Number);
    year = y;
    month = m - 1;
    mode = "month";
  }

  function openTaskWindow(id: number) {
    invoke("open_task_window", { taskId: id });
  }

  function onRowClick(t: TaskRow) {
    if (t.source_id !== null && t.source_day) openDay(t.source_day);
    else openTaskWindow(t.id);
  }

  const monthDays = $derived(
    Array.from(
      { length: new Date(year, month + 1, 0).getDate() },
      (_, i) => dateStr(new Date(year, month, i + 1)),
    ),
  );

  function startTransfer(t: TaskRow, e: MouseEvent) {
    e.stopPropagation();
    transferring = transferring?.id === t.id ? null : t;
  }

  async function doTransfer(targetDay: string) {
    if (!db || !transferring) return;
    const src = transferring;
    transferring = null;
    if (targetDay === src.day) return;

    try {
      const now = new Date().toISOString();
      const rowsAffected = await db.execute(
        `INSERT INTO tasks (day, position, text, source_id, created_at, updated_at)
         SELECT $1,
                (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE day = $1),
                '', $2, $3, $3
         WHERE NOT EXISTS (
           SELECT 1 FROM tasks WHERE source_id = $2 AND day = $1
         )`,
        [targetDay, src.id, now],
      );
      if (rowsAffected === 0) {
        // дубль — молча игнорируем
      }
      await loadMonth();
    } catch (e) {
      console.error("Не удалось перенести задачу:", e);
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (transferring) transferring = null;
      else getCurrentWindow().hide();
    }
  }

  onMount(() => {
    getDb().then(async (loaded) => {
      db = loaded;
      await loadYear();
    });

    const un = listen("tasks-changed", () => {
      if (!db) return;
      if (mode === "year") void loadYear();
      else void loadMonth();
    });
    return () => void un.then((f) => f());
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
            onclick={() => onRowClick(t)}
            onkeydown={(e) => e.key === "Enter" && onRowClick(t)}
          >
            <span class="num">{t.position}.</span>
            <span class="text">{t.text}{t.truncated ? "…" : ""}</span>
            <span class="actions">
              {#if t.source_id !== null && t.source_day}
                <span class="src">↗ {shortDayLabel(t.source_day)}</span>
              {/if}
              <button
                class="iconbtn"
                class:lit={t.comments > 0}
                title="Комментарии ({t.comments})"
                aria-label="Комментарии: {t.comments}"
                onclick={(e) => {
                  e.stopPropagation();
                  openTaskWindow(t.source_id ?? t.id);
                }}
              >💬&thinsp;<span class="count">{t.comments}</span></button>
              {#if t.source_id === null && transferring?.id !== t.id}
                <button
                  class="iconbtn"
                  title="Перенести в другой день"
                  aria-label="Перенести"
                  onclick={(e) => startTransfer(t, e)}
                >↪</button>
              {/if}
            </span>
          </div>

          {#if transferring?.id === t.id}
            <div class="picker">
              <span class="picker-label">Перенести в {cap(MONTHS_RU[month])}:</span>
              {#each monthDays as md}
                <button
                  class="chip"
                  class:current={md === t.day}
                  disabled={md === t.day}
                  style:background={heat(md)}
                  onclick={() => doTransfer(md)}
                >{Number(md.slice(-2))}</button>
              {/each}
              <button class="chip cancel" onclick={() => (transferring = null)}>
                ✕
              </button>
            </div>
          {/if}
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
    flex-shrink: 0;
    font-size: 11px;
    color: var(--accent);
    white-space: nowrap;
  }

  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .iconbtn {
    border: none;
    background: none;
    font-size: 11px;
    line-height: 1;
    color: var(--muted);
    padding: 3px 6px;
    border-radius: 9px;
    cursor: pointer;
    opacity: 0;
    white-space: nowrap;
  }

  .task:hover .iconbtn,
  .iconbtn.lit {
    opacity: 1;
  }

  .iconbtn.lit {
    color: var(--accent);
  }

  .iconbtn:hover {
    background: var(--hover);
    color: var(--accent);
  }

  .count {
    font-variant-numeric: tabular-nums;
  }

  .picker {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px;
    padding: 6px 8px 10px;
  }

  .picker-label {
    width: 100%;
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 2px;
  }

  .chip {
    min-width: 24px;
    height: 24px;
    border: none;
    border-radius: 6px;
    background: var(--hover);
    color: var(--ink);
    font-size: 11px;
    cursor: pointer;
  }

  .chip:hover:not(:disabled) {
    outline: 1.5px solid var(--accent);
    outline-offset: -1.5px;
  }

  .chip:disabled,
  .chip.current {
    opacity: 0.35;
    cursor: default;
  }

  .chip.cancel {
    color: var(--muted);
  }

  .none {
    margin-top: 40px;
    text-align: center;
    color: var(--muted);
  }
</style>
