<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { getDb } from "./lib/db";
  import {
    WEEKDAYS,
    MONTHS_RU,
    buildMonthCells,
    cap,
    dateStr,
    dayLabel,
    monthBounds,
    shortDayLabel,
    taskCountWord,
    yearBounds,
  } from "./lib/dates";
  import {
    insertTransfer,
    loadMonthTasks,
    loadRangeCounts,
    loadTransferredDays,
    setTaskDone,
    type TaskRow,
  } from "./lib/tasks";

  type Mode = "year" | "month";

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let mode: Mode = $state("year");
  let year: number = $state(new Date().getFullYear());
  let month: number = $state(new Date().getMonth());
  let counts: Record<string, number> = $state({});
  let rows: TaskRow[] = $state([]);
  let loading = $state(false);
  let error = $state("");

  let transferring: TaskRow | null = $state(null);
  let pickYear: number = $state(new Date().getFullYear());
  let pickMonth: number = $state(new Date().getMonth());
  let pickCounts: Record<string, number> = $state({});
  let transferredDays: Set<string> = $state(new Set());

  let pendingDone: Set<number> = $state(new Set());

  let tooltip: { text: string; x: number; y: number } | null = $state(null);
  let tooltipTimer: ReturnType<typeof setTimeout> | null = $state(null);

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
    for (const row of rows) {
      const list = map.get(row.day);
      if (list) list.push(row);
      else map.set(row.day, [row]);
    }
    return [...map.entries()];
  });

  const pickDays = $derived.by(() => {
    const total = new Date(pickYear, pickMonth + 1, 0).getDate();
    return Array.from({ length: total }, (_, i) =>
      dateStr(new Date(pickYear, pickMonth, i + 1)),
    );
  });

  // защита от stale-ответов при быстром переключении режимов/месяцев
  let loadTicket = 0;

  function heat(day: string, source: Record<string, number> = counts): string {
    const n = source[day] ?? 0;
    if (!n) return "transparent";
    const max = Object.values(source).reduce((a, b) => Math.max(a, b), 0);
    const alpha = Math.round(
      (0.14 + 0.86 * Math.min(1, n / Math.max(1, max))) * 100,
    );
    return `color-mix(in srgb, var(--accent) ${alpha}%, transparent)`;
  }

  async function loadYear() {
    if (!db) return;
    const myYear = year;
    const ticket = ++loadTicket;
    loading = true;
    try {
      const [from, to] = yearBounds(myYear);
      const next = await loadRangeCounts(db, from, to);
      if (ticket !== loadTicket || mode !== "year" || year !== myYear) return;
      counts = next;
      error = "";
    } catch (e) {
      if (ticket === loadTicket) error = `Не удалось загрузить год: ${String(e)}`;
    } finally {
      if (ticket === loadTicket) loading = false;
    }
  }

  async function loadMonth() {
    if (!db) return;
    const [myYear, myMonth] = [year, month];
    const ticket = ++loadTicket;
    loading = true;
    rows = [];
    try {
      const [from, to] = monthBounds(myYear, myMonth);
      const next = await loadMonthTasks(db, from, to);
      if (
        ticket !== loadTicket ||
        mode !== "month" ||
        year !== myYear ||
        month !== myMonth
      ) {
        return;
      }
      rows = next;
      error = "";
    } catch (e) {
      if (ticket === loadTicket) {
        error = `Не удалось загрузить месяц: ${String(e)}`;
      }
    } finally {
      if (ticket === loadTicket) loading = false;
    }
  }

  $effect(() => {
    if (!db) return;
    if (mode === "year") void loadYear();
    else void loadMonth();
  });

  // данные для пикера переноса: счётчики целевого месяца и уже занятые дни
  $effect(() => {
    const database = db;
    const source = transferring;
    if (!database || !source) return;
    const myYear = pickYear;
    const myMonth = pickMonth;
    let cancelled = false;
    (async () => {
      try {
        const [from, to] = monthBounds(myYear, myMonth);
        const [nextCounts, days] = await Promise.all([
          loadRangeCounts(database, from, to),
          loadTransferredDays(database, source.id),
        ]);
        if (cancelled) return;
        pickCounts = nextCounts;
        transferredDays = new Set(days);
      } catch (e) {
        if (!cancelled) {
          error = `Не удалось подготовить перенос: ${String(e)}`;
        }
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  function step(dir: number) {
    transferring = null;
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
    transferring = null;
    year = y;
    month = m - 1;
    mode = "month";
  }

  function openTaskWindow(id: number) {
    void invoke("open_task_window", { taskId: id });
  }

  function onRowClick(task: TaskRow) {
    if (task.source_id !== null && task.source_day) openDay(task.source_day);
    else openTaskWindow(task.source_id ?? task.id);
  }

  async function toggleDone(task: TaskRow) {
    if (!db || pendingDone.has(task.id)) return;
    pendingDone = new Set(pendingDone).add(task.id);
    try {
      await setTaskDone(db, task);
      emit("tasks-changed");
    } catch (e) {
      error = `Не удалось изменить статус: ${String(e)}`;
    } finally {
      const next = new Set(pendingDone);
      next.delete(task.id);
      pendingDone = next;
    }
  }

  function startTransfer(task: TaskRow) {
    if (transferring?.id === task.id) {
      transferring = null;
      return;
    }
    transferring = task;
    pickYear = year;
    pickMonth = month;
  }

  function shiftPickMonth(dir: number) {
    const d = new Date(pickYear, pickMonth + dir, 1);
    pickYear = d.getFullYear();
    pickMonth = d.getMonth();
  }

  async function doTransfer(targetDay: string) {
    const source = transferring;
    if (!db || !source) return;
    if (targetDay === source.day) return;
    transferring = null;
    try {
      await insertTransfer(db, targetDay, source.id);
      const [targetYear, targetMonth] = targetDay.split("-").map(Number);
      if (targetYear !== year || targetMonth - 1 !== month) {
        year = targetYear;
        month = targetMonth - 1;
        mode = "month";
      }
      emit("tasks-changed");
    } catch (e) {
      error = `Не удалось перенести задачу: ${String(e)}`;
    }
  }

  function scheduleTooltip(day: string, el: HTMLElement) {
    if (tooltipTimer) clearTimeout(tooltipTimer);
    tooltipTimer = setTimeout(() => {
      const n = counts[day] ?? 0;
      if (n === 0) return;
      const rect = el.getBoundingClientRect();
      tooltip = {
        text: `${dayLabel(day)}: ${n} ${taskCountWord(n)}`,
        x: rect.left + rect.width / 2,
        y: rect.top - 8,
      };
    }, 1000);
  }

  function showTooltipNow(day: string, el: HTMLElement) {
    if (tooltipTimer) {
      clearTimeout(tooltipTimer);
      tooltipTimer = null;
    }
    const n = counts[day] ?? 0;
    if (n === 0) {
      hideTooltip();
      return;
    }
    const rect = el.getBoundingClientRect();
    tooltip = {
      text: `${dayLabel(day)}: ${n} ${taskCountWord(n)}`,
      x: rect.left + rect.width / 2,
      y: rect.top - 8,
    };
  }

  function hideTooltip() {
    if (tooltipTimer) {
      clearTimeout(tooltipTimer);
      tooltipTimer = null;
    }
    tooltip = null;
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (transferring) {
        transferring = null;
        return;
      }
      hideTooltip();
      void getCurrentWindow().hide();
    }
  }

  onMount(() => {
    let disposed = false;

    getDb()
      .then((loaded) => {
        if (!disposed) db = loaded;
      })
      .catch((e) => {
        if (!disposed) error = `Не удалось открыть базу данных: ${String(e)}`;
      });

    const unlistenTasks = listen("tasks-changed", () => {
      if (disposed || !db) return;
      if (mode === "year") void loadYear();
      else void loadMonth();
    });

    const onVisibility = () => {
      if (document.hidden) hideTooltip();
    };
    document.addEventListener("visibilitychange", onVisibility);

    // окно могли скрыть/показать — подстрахуемся от устаревшего списка и тултипа
    const onFocus = () => {
      hideTooltip();
      if (disposed || !db) return;
      if (mode === "year") void loadYear();
      else void loadMonth();
    };
    const onBlur = () => hideTooltip();
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);

    return () => {
      disposed = true;
      if (tooltipTimer) clearTimeout(tooltipTimer);
      document.removeEventListener("visibilitychange", onVisibility);
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
      void unlistenTasks.then((f) => f());
    };
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
    <div class="seg" role="group" aria-label="Режим календаря">
      <button
        class:active={mode === "month"}
        aria-pressed={mode === "month"}
        onclick={() => {
          transferring = null;
          mode = "month";
        }}
      >
        Месяц
      </button>
      <button
        class:active={mode === "year"}
        aria-pressed={mode === "year"}
        onclick={() => {
          transferring = null;
          mode = "year";
        }}
      >
        Год
      </button>
    </div>
  </header>

  {#if error}
    <p class="banner" role="alert">{error}</p>
  {/if}

  {#if mode === "year"}
    <div class="year-scroll">
      <section class="year-grid" aria-busy={loading}>
      {#each monthsGrid as { m, cells }}
        <div class="month">
          <button
            class="month-name"
            onclick={() =>
              openDay(`${year}-${String(m + 1).padStart(2, "0")}-01`)}
          >
            {cap(MONTHS_RU[m])}
          </button>
          <div class="wd" aria-hidden="true">
            {#each WEEKDAYS as w}<span>{w}</span>{/each}
          </div>
          <div class="cells">
            {#each cells as cell}
              {#if cell}
                {@const n = counts[cell] ?? 0}
                <button
                  class="cell"
                  class:has-tasks={n > 0}
                  style:background={heat(cell)}
                  aria-label={`${dayLabel(cell)}: ${n > 0 ? `${n} ${taskCountWord(n)}` : "нет задач"}`}
                  onclick={() => openDay(cell)}
                  onmouseenter={(e) => scheduleTooltip(cell, e.currentTarget)}
                  onmouseleave={hideTooltip}
                  onfocus={(e) => showTooltipNow(cell, e.currentTarget)}
                  onblur={hideTooltip}
                >{Number(cell.slice(-2))}</button>
              {:else}
                <span class="cell empty" aria-hidden="true"></span>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
      </section>
    </div>
  {:else}
    <section class="month-list" aria-busy={loading}>
      {#each daysGrouped as [d, list]}
        <h3 class="day-head">{dayLabel(d)}</h3>
        {#each list as task (task.id)}
          <div
            class="task"
            class:transfer={task.source_id !== null}
            class:done={task.is_done}
          >
            <button
              class="task-main"
              title={task.source_id !== null
                ? "Перейти к оригиналу"
                : "Открыть задачу и комментарии"}
              onclick={() => onRowClick(task)}
            >
              <span class="num">{task.position}.</span>
              <span class="text">{task.text}{task.truncated ? "…" : ""}</span>
            </button>
            {#if task.source_id !== null && task.source_day}
              <span class="src" title="Перенос из дня">
                ↗ {shortDayLabel(task.source_day)}
              </span>
            {/if}
            {#if task.transfers > 0}
              <span
                class="has-transfers"
                title="У этой задачи есть переносы"
                aria-label="У этой задачи есть переносы"
              ></span>
            {/if}
            <span class="actions">
              <button
                class="done-btn"
                class:done={task.is_done}
                title={task.is_done
                  ? "Вернуть в работу"
                  : "Отметить выполненной"}
                aria-label={task.is_done
                  ? "Вернуть в работу"
                  : "Отметить выполненной"}
                onclick={(e) => {
                  e.stopPropagation();
                  void toggleDone(task);
                }}
              >{task.is_done ? "↩" : "✓"}</button>
              <button
                class="iconbtn"
                class:lit={task.comments > 0}
                title="Комментарии ({task.comments})"
                aria-label="Комментарии: {task.comments}"
                onclick={(e) => {
                  e.stopPropagation();
                  openTaskWindow(task.source_id ?? task.id);
                }}
              >💬&thinsp;<span class="count">{task.comments}</span></button>
              {#if task.source_id === null}
                <button
                  class="iconbtn"
                  title="Перенести в другой день"
                  aria-label="Перенести в другой день"
                  onclick={(e) => {
                    e.stopPropagation();
                    startTransfer(task);
                  }}
                >↪</button>
              {/if}
            </span>
          </div>

          {#if transferring?.id === task.id}
            <div class="picker">
              <div class="picker-head">
                <button
                  class="navbtn small"
                  onclick={() => shiftPickMonth(-1)}
                  aria-label="Предыдущий месяц"
                >‹</button>
                <span class="picker-label">
                  Перенести в {cap(MONTHS_RU[pickMonth])} {pickYear}:
                </span>
                <button
                  class="navbtn small"
                  onclick={() => shiftPickMonth(1)}
                  aria-label="Следующий месяц"
                >›</button>
              </div>
              <div class="chips">
                {#each pickDays as md}
                  {@const already = transferredDays.has(md)}
                  <button
                    class="chip"
                    class:current={md === task.day}
                    disabled={md === task.day || already}
                    title={already
                      ? "Уже перенесено в этот день"
                      : md === task.day
                        ? "Это день задачи"
                        : md}
                    style:background={heat(md, pickCounts)}
                    onclick={() => doTransfer(md)}
                  >{Number(md.slice(-2))}</button>
                {/each}
              </div>
              <button
                class="chip cancel"
                aria-label="Отменить перенос"
                onclick={() => (transferring = null)}
              >✕</button>
            </div>
          {/if}
        {/each}
      {/each}
      {#if !error && !loading && rows.length === 0}
        <p class="none">В этом месяце задач нет</p>
      {/if}
    </section>
  {/if}

  {#if tooltip}
    <div
      class="year-tooltip"
      style:left="{tooltip.x}px"
      style:top="{tooltip.y}px"
      style:transform="translate(-50%, -100%)"
    >{tooltip.text}</div>
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
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 8px 12px;
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

  .navbtn.small {
    font-size: 16px;
    padding: 0 6px;
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

  .banner {
    margin: 0 18px 8px;
    padding: 6px 10px;
    border-radius: 7px;
    background: color-mix(in srgb, #d24c3f 14%, var(--bg));
    color: #d24c3f;
    font-size: 12px;
    line-height: 1.35;
  }

  .year-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 12px 14px;
  }

  /* ВАЖНО: в сетке года намеренно НЕТ grid gap и container queries —
     WebKit теряет grid gap после увеличения + повторного сужения окна,
     если в поддереве есть containment/container-type (проверено на живом
     приложении). Зазоры сделаны структурно: padding у месяца и margin
     у ячеек дня. 188px — контент месяца (7 ячеек), + 31px padding =
     219px трек; 3 × 219px = 657px минимальная ширина года. */
  .year-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(219px, 1fr));
    gap: 0;
    min-width: 657px;
  }

  .year-tooltip {
    position: fixed;
    z-index: 1000;
    pointer-events: none;
    padding: 5px 9px;
    border-radius: 6px;
    background: var(--ink);
    color: var(--bg);
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
  }

  .month {
    /* зазор между месяцами: 31px по горизонтали и 29px по вертикали
       (половина — с каждой стороны); не зависит от grid gap */
    padding: 14.5px 15.5px;
  }

  .month-name {
    border: none;
    background: none;
    padding: 0 0 2px;
    color: var(--ink);
    font-size: clamp(11px, 1.4vw, 16px);
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
    font-size: clamp(9px, 1.1vw, 12px);
    color: var(--muted);
    text-align: center;
  }

  .cells {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    /* не grid gap: зазор даёт margin ячейки, чтобы WebKit не «терял» его
       при ресайзе окна */
    gap: 0;
  }

  .cell {
    box-sizing: border-box;
    /* у нативной кнопки есть padding — без сброса min-content ячейки
       становится шире трека и колонка «наезжает» на соседний месяц */
    padding: 0;
    margin: 1px;
    -webkit-appearance: none;
    appearance: none;
    aspect-ratio: 1;
    border: none;
    border-radius: clamp(3px, 0.45vw, 7px);
    background: transparent;
    color: var(--ink);
    font-size: clamp(12px, 2.2vw, 20px);
    line-height: 1;
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
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1.5px var(--accent);
    margin-bottom: 5px;
  }

  .task:hover {
    background: var(--hover);
  }

  .task.transfer {
    background: color-mix(in srgb, var(--accent) 16%, var(--bg));
  }

  .task.done {
    box-shadow: inset 0 0 0 1.5px var(--success, #34c759);
  }

  .task.done .text {
    color: var(--muted);
    text-decoration: line-through;
  }

  .task-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0;
    border: none;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
  }

  .num {
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .text {
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
    user-select: text;
  }

  .src {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--accent);
    white-space: nowrap;
  }

  .has-transfers {
    flex-shrink: 0;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    opacity: 0.75;
  }

  .actions {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }

  .done-btn {
    width: 22px;
    height: 22px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--success, #34c759);
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.12s ease;
  }

  .task:hover .done-btn,
  .done-btn:focus-visible {
    opacity: 1;
  }

  .done-btn:hover {
    background: var(--hover);
  }

  .done-btn.done {
    opacity: 1;
    color: var(--muted);
  }

  .done-btn.done:hover {
    color: var(--ink);
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
  .iconbtn.lit,
  .iconbtn:focus-visible {
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
    padding: 6px 8px 10px;
  }

  .picker-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-bottom: 4px;
  }

  .picker-label {
    font-size: 11px;
    color: var(--muted);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 3px;
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
    margin-top: 6px;
    color: var(--muted);
  }

  .none {
    margin-top: 40px;
    text-align: center;
    color: var(--muted);
  }

  .navbtn:focus-visible,
  .today:focus-visible,
  .seg button:focus-visible,
  .month-name:focus-visible,
  .cell:focus-visible,
  .task-main:focus-visible,
  .done-btn:focus-visible,
  .iconbtn:focus-visible,
  .chip:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
