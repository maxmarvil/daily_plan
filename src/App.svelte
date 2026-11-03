<script lang="ts">
  import { onMount, tick } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { emit, listen } from "@tauri-apps/api/event";
  import { getDb } from "./lib/db";
  import {
    insertTask,
    loadDayTasks,
    setTaskDone,
    updateTaskText,
    type TaskRow,
  } from "./lib/tasks";
  import {
    SETTING_CAPTURE_SHORTCUT,
    SETTING_NEW_TASK_SHORTCUT,
    getSetting,
  } from "./lib/settings";
  import { combosEqual } from "./lib/shortcut";
  import { shortDayLabel, todayStr } from "./lib/dates";

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let tasks: TaskRow[] = $state([]);
  let day = $state(todayStr());

  let adding = $state(false);
  let draft = $state("");
  let editing: { id: number; text: string } | null = $state(null);
  let addEditor: HTMLTextAreaElement | null = $state(null);

  let saving = $state(false);
  let pendingDone: Set<number> = $state(new Set());
  let error = $state("");

  // защита от stale-ответов при быстрых переключениях/обновлениях
  let loadTicket = 0;

  // автофокус для полей ввода
  function focus(el: HTMLTextAreaElement) {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }

  async function refresh() {
    if (!db) return;
    const ticket = ++loadTicket;
    try {
      const rows = await loadDayTasks(db, day);
      if (ticket !== loadTicket) return;
      tasks = rows;
      error = "";
    } catch (e) {
      if (ticket !== loadTicket) return;
      error = `Не удалось загрузить задачи: ${String(e)}`;
    }
  }

  /** Смена суток (в т.ч. после сна, когда таймер мог не сработать). */
  function syncDay(): boolean {
    const today = todayStr();
    if (today === day) return false;
    day = today;
    adding = false;
    editing = null;
    draft = "";
    return true;
  }

  async function commitDraft() {
    if (saving) return;
    const text = draft.trim();
    if (!db || !text) {
      adding = false;
      draft = "";
      return;
    }
    saving = true;
    // дата берётся на момент сохранения: не пишем во вчера после полуночи/сна
    const targetDay = todayStr();
    if (targetDay !== day) day = targetDay;
    try {
      const affected = await insertTask(db, targetDay, text);
      if (affected === 0) {
        error = "Не удалось сохранить задачу";
        return;
      }
      adding = false;
      draft = "";
      await refresh();
      emit("tasks-changed");
    } catch (e) {
      // не теряем набранное: поле остаётся открытым
      error = `Не удалось сохранить задачу: ${String(e)}`;
    } finally {
      saving = false;
    }
  }

  function startEdit(task: TaskRow) {
    if (task.source_id !== null) {
      openTaskWindow(task);
      return;
    }
    editing = { id: task.id, text: task.text };
  }

  async function saveEdit() {
    if (!db || !editing || saving) return;
    const current = editing;
    const text = current.text.trim();
    saving = true;
    try {
      if (text) await updateTaskText(db, current.id, text);
      // сбрасываем только свой редактор: пользователь мог открыть другой
      if (editing?.id === current.id) editing = null;
      await refresh();
      emit("tasks-changed");
    } catch (e) {
      error = `Не удалось сохранить задачу: ${String(e)}`;
    } finally {
      saving = false;
    }
  }

  async function toggleDone(task: TaskRow) {
    if (!db || pendingDone.has(task.id)) return;
    pendingDone = new Set(pendingDone).add(task.id);
    try {
      await setTaskDone(db, task);
      await refresh();
      emit("tasks-changed");
    } catch (e) {
      error = `Не удалось изменить статус: ${String(e)}`;
    } finally {
      const next = new Set(pendingDone);
      next.delete(task.id);
      pendingDone = next;
    }
  }

  function openTaskWindow(task: TaskRow) {
    void invoke("open_task_window", { taskId: task.source_id ?? task.id });
  }

  function onInputKeydown(e: KeyboardEvent, commit: () => void) {
    // Enter — зафиксировать, Shift+Enter — перевод строки
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      commit();
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (adding || editing) {
        adding = false;
        editing = null;
        draft = "";
      } else {
        void getCurrentWindow().hide();
      }
    }
  }

  async function applyStoredShortcuts(loaded: NonNullable<typeof db>) {
    const shortcuts: {
      key: string;
      read: string;
      apply: string;
    }[] = [
      {
        key: SETTING_CAPTURE_SHORTCUT,
        read: "get_shortcut",
        apply: "set_shortcut",
      },
      {
        key: SETTING_NEW_TASK_SHORTCUT,
        read: "get_new_task_shortcut",
        apply: "set_new_task_shortcut",
      },
    ];
    for (const shortcut of shortcuts) {
      try {
        const stored = await getSetting(loaded, shortcut.key);
        if (!stored) continue;
        const current = await invoke<string>(shortcut.read);
        if (!combosEqual(stored, current)) {
          await invoke(shortcut.apply, { shortcut: stored });
        }
      } catch (e) {
        console.error("Не удалось применить сохранённую комбинацию:", e);
      }
    }
  }

  /** Раз в день — консистентный снимок БД (VACUUM INTO корректно работает с WAL). */
  async function ensureDailyBackup(loaded: NonNullable<typeof db>) {
    try {
      const path = await invoke<string | null>("prepare_backup", {
        day: todayStr(),
      });
      if (!path) return;
      await loaded.execute("VACUUM INTO $1", [path]);
      await invoke("prune_backups", { keep: 7 });
    } catch (e) {
      console.error("Не удалось создать бэкап БД:", e);
    }
  }

  onMount(() => {
    let disposed = false;

    getDb()
      .then(async (loaded) => {
        if (disposed) return;
        db = loaded;
        await applyStoredShortcuts(loaded);
        await refresh();
        void ensureDailyBackup(loaded);
      })
      .catch((e) => {
        if (!disposed) error = `Не удалось открыть базу данных: ${String(e)}`;
      });

    const onShown = async () => {
      if (!syncDay() && editing) {
        // глобальный шорткат не присылает blur — сохраняем правку, а не теряем текст
        await saveEdit();
      }
      if (!editing) {
        adding = true;
        await tick();
        addEditor?.focus();
      }
      await refresh();
    };

    // при вызове окна по горячей клавише сразу даём поле ввода
    const unlistenCapture = listen("capture-shown", onShown);
    const unlistenNewTask = listen("new-task-shortcut", onShown);
    // изменения из календаря/окна задачи — обновляем список
    const unlistenTasks = listen("tasks-changed", () => {
      if (!disposed) void refresh();
    });

    // смена полуночи: если день изменился, переключаемся на новый
    const timer = setInterval(() => {
      if (syncDay()) void refresh();
    }, 30_000);

    return () => {
      disposed = true;
      clearInterval(timer);
      void unlistenCapture.then((f) => f());
      void unlistenNewTask.then((f) => f());
      void unlistenTasks.then((f) => f());
    };
  });

  const dateLabel = $derived(
    new Date(`${day}T12:00:00`).toLocaleDateString("ru-RU", {
      day: "numeric",
      month: "long",
      weekday: "short",
    }),
  );
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main class="capture">
  <header class="day-label">
    <span>{dateLabel}</span>
    <span class="hdr-actions">
      <button
        class="cal"
        aria-label="Настройки"
        title="Настройки"
        onclick={() => invoke("open_settings")}
      >
        <svg viewBox="0 0 20 20" width="30" height="30" aria-hidden="true">
          <path
            d="M10 7a3 3 0 1 0 0 6 3 3 0 0 0 0-6z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
          />
          <path
            d="M16.9 11.2V8.8l-2.2-.3a6.2 6.2 0 0 0-.6-1.3l1.3-1.8-1.7-1.7-1.8 1.3a6.2 6.2 0 0 0-1.3-.6l-.3-2.2H8.2l-.3 2.2a6.2 6.2 0 0 0-1.3.6l-1.8-1.3-1.7 1.7 1.3 1.8a6.2 6.2 0 0 0-.6 1.3l-2.2.3v2.4l2.2.3c.1.5.3.9.6 1.3l-1.3 1.8 1.7 1.7 1.8-1.3c.4.3.8.5 1.3.6l.3 2.2h2.4l.3-2.2c.5-.1.9-.3 1.3-.6l1.8 1.3 1.7-1.7-1.3-1.8c.3-.4.5-.8.6-1.3l2.2-.3z"
            fill="none"
            stroke="currentColor"
            stroke-width="1.4"
            stroke-linejoin="round"
          />
        </svg>
      </button>
      <button
        class="cal"
        aria-label="Открыть календарь"
        title="Календарь"
        onclick={() => invoke("open_calendar")}
      >
        <svg viewBox="0 0 20 20" width="30" height="30" aria-hidden="true">
          <path
            d="M3 6A2 2 0 0 1 5 4h10a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM3 8h14M7 2.5V5M13 2.5V5"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
          />
        </svg>
      </button>
    </span>
  </header>

  {#if error}
    <p class="banner" role="alert">{error}</p>
  {/if}

  <section class="list" aria-label="Задачи на сегодня">
    {#each tasks as task (task.id)}
      {#if editing?.id === task.id}
        <textarea
          class="editor"
          use:focus
          rows="3"
          aria-label="Редактирование задачи"
          bind:value={editing.text}
          onkeydown={(e) => onInputKeydown(e, saveEdit)}
          onblur={saveEdit}
        ></textarea>
      {:else}
        <div
          class="task"
          class:transfer={task.source_id !== null}
          class:done={task.is_done}
        >
          <button
            class="task-main"
            title={task.source_id !== null
              ? "Открыть оригинал задачи"
              : "Редактировать задачу"}
            onclick={() => startEdit(task)}
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
              class="iconbtn"
              class:lit={task.comments > 0}
              title="Комментарии ({task.comments})"
              aria-label="Комментарии: {task.comments}"
              onclick={(e) => {
                e.stopPropagation();
                openTaskWindow(task);
              }}
            >
              💬{#if task.comments > 0}<span class="count">{task.comments}</span>{/if}
            </button>
            <button
              class="done-btn"
              class:done={task.is_done}
              aria-label={task.is_done
                ? "Вернуть в работу"
                : "Отметить выполненной"}
              title={task.is_done ? "Вернуть в работу" : "Отметить выполненной"}
              onclick={(e) => {
                e.stopPropagation();
                void toggleDone(task);
              }}
            >
              {task.is_done ? "↩" : "✓"}
            </button>
          </span>
        </div>
      {/if}
    {/each}

    {#if !error && db && !adding && tasks.length === 0}
      <p class="none">На сегодня задач нет</p>
    {/if}

    {#if adding}
      <textarea
        class="editor new"
        use:focus
        bind:this={addEditor}
        rows="3"
        aria-label="Новая задача"
        placeholder="Новая задача…"
        bind:value={draft}
        onkeydown={(e) => onInputKeydown(e, commitDraft)}
        onblur={commitDraft}
      ></textarea>
    {:else}
      <div class="bar">
        <button
          class="add"
          aria-label="Добавить задачу"
          onclick={() => (adding = true)}
        >+</button>
      </div>
    {/if}
  </section>
</main>

<style>
  .capture {
    display: flex;
    flex-direction: column;
    height: 100vh;
    font-family:
      -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif;
    font-size: 15px;
    line-height: 1.45;
    color: var(--ink);
    background: var(--bg);
  }

  .day-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 18px 6px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
    flex-shrink: 0;
  }

  .cal {
    border: none;
    background: none;
    padding: 4px;
    border-radius: 6px;
    color: var(--muted);
    display: flex;
    cursor: pointer;
  }

  .cal:hover {
    background: var(--hover);
    color: var(--accent);
  }

  .hdr-actions {
    display: flex;
    gap: 2px;
  }

  .banner {
    margin: 4px 12px 6px;
    padding: 6px 10px;
    border-radius: 7px;
    background: color-mix(in srgb, #d24c3f 14%, var(--bg));
    color: #d24c3f;
    font-size: 12px;
    line-height: 1.35;
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 10px;
  }

  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1.5px var(--accent);
    margin-bottom: 5px;
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

  .task:hover {
    background: var(--hover);
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
    user-select: none;
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
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 4px;
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

  .done-btn {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    border: none;
    border-radius: 6px;
    background: transparent;
    color: var(--success, #34c759);
    font-size: 14px;
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

  .editor {
    box-sizing: border-box;
    width: 100%;
    margin: 4px 0;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: var(--editor-bg);
    color: var(--ink);
    font: inherit;
    resize: vertical;
    outline: none;
    box-shadow: 0 0 0 1.5px var(--accent);
  }

  .bar {
    display: flex;
    justify-content: center;
    padding: 6px 8px 10px;
  }

  .add {
    --add-h: 45px;
    width: calc(var(--add-h) * 4);
    height: var(--add-h);
    border: none;
    border-radius: 9999px;
    background: var(--accent);
    color: #fff;
    font-size: 24px;
    line-height: 1;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .none {
    margin: 18px 0 8px;
    text-align: center;
    color: var(--muted);
    font-size: 13px;
  }

  .cal:focus-visible,
  .task-main:focus-visible,
  .iconbtn:focus-visible,
  .done-btn:focus-visible,
  .add:focus-visible,
  .editor:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
