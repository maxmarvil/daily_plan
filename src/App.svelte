<script lang="ts">
  import { onMount } from "svelte";
  import Database from "@tauri-apps/plugin-sql";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import { todayStr } from "./lib/dates";

  type Task = {
    id: number;
    day: string;
    position: number;
    text: string;
    is_transfer: number;
    source_id: number | null;
  };

  let db: Database | null = $state(null);
  let tasks: Task[] = $state([]);
  let day = $state(todayStr());

  let adding = $state(false);
  let draft = $state("");
  let editing: { id: number; text: string } | null = $state(null);

  // автофокус для полей ввода
  function focus(el: HTMLTextAreaElement) {
    el.focus();
    el.setSelectionRange(el.value.length, el.value.length);
  }

  async function refresh() {
    if (!db) return;
    tasks = await db.select<Task[]>(
      `SELECT id, day, position, text, source_id, is_transfer
       FROM tasks
       WHERE day = $1
       ORDER BY position`,
      [day],
    );
  }

  async function commitDraft() {
    const text = draft.trim();
    if (!db || !text) {
      adding = false;
      draft = "";
      return;
    }
    try {
      const rows = await db.select<{ max: number | null }[]>(
        `SELECT MAX(position) AS max FROM tasks WHERE day = $1`,
        [day],
      );
      const position = (rows[0]?.max ?? 0) + 1;
      const now = new Date().toISOString();
      await db.execute(
        `INSERT INTO tasks (day, position, text, created_at, updated_at)
         VALUES ($1, $2, $3, $4, $5)`,
        [day, position, text, now, now],
      );
      adding = false;
      draft = "";
      await refresh();
      emit("tasks-changed");
    } catch (e) {
      // не теряем набранное: поле остаётся открытым
      console.error("Не удалось сохранить задачу:", e);
    }
  }

  function startEdit(task: Task) {
    editing = { id: task.id, text: task.text };
  }

  async function saveEdit() {
    if (!db || !editing) return;
    const text = editing.text.trim();
    if (text) {
      await db.execute(
        `UPDATE tasks SET text = $1, updated_at = $2 WHERE id = $3`,
        [text, new Date().toISOString(), editing.id],
      );
    }
    editing = null;
    await refresh();
    emit("tasks-changed");
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
        getCurrentWindow().hide();
      }
    }
  }

  async function applyStoredShortcut(loaded: Database) {
    try {
      const stored = await loaded.select<{ value: string }[]>(
        `SELECT value FROM settings WHERE key = 'capture_shortcut'`,
      );
      const want = stored[0]?.value;
      if (want && want !== (await invoke<string>("get_shortcut"))) {
        await invoke("set_shortcut", { shortcut: want });
      }
    } catch (e) {
      console.error("Не удалось применить сохранённую комбинацию:", e);
    }
  }

  onMount(() => {
    Database.load("sqlite:daily-plan.db").then(async (loaded) => {
      db = loaded;
      await applyStoredShortcut(loaded);
      await refresh();
    });

    // смена полуночи: если день изменился, переключаемся на новый
    const timer = setInterval(() => {
      const t = todayStr();
      if (t !== day) {
        day = t;
        refresh();
      }
    }, 30_000);
    return () => clearInterval(timer);
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
        <svg viewBox="0 0 20 20" width="15" height="15" aria-hidden="true">
          <path
            d="M10 12.6a2.6 2.6 0 1 0 0-5.2 2.6 2.6 0 0 0 0 5.2zM10 2.8l1.4 2.1h2.4l.6 2.4 2 1.4-1 2.2.7 2.4-2.3.8L12.5 18 10 16.9l-2.5 1.1-1.3-2-2.3-.8.7-2.4-1-2.2 2-1.4.6-2.4h2.4z"
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
        <svg viewBox="0 0 20 20" width="15" height="15" aria-hidden="true">
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

  <section class="list">
    {#each tasks as task, i}
      {#if editing?.id === task.id}
        <textarea
          class="editor"
          use:focus
          rows="3"
          bind:value={editing.text}
          onkeydown={(e) => onInputKeydown(e, saveEdit)}
          onblur={saveEdit}
        ></textarea>
      {:else}
        <div
          class="task"
          role="button"
          tabindex="0"
          onclick={() => startEdit(task)}
          onkeydown={(e) => e.key === "Enter" && startEdit(task)}
        >
          <span class="num">{i + 1}.</span>
          <span class="text">{task.text}</span>
        </div>
      {/if}
    {/each}

    {#if adding}
      <textarea
        class="editor new"
        use:focus
        rows="3"
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

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 10px;
  }

  .task {
    display: flex;
    gap: 8px;
    padding: 7px 8px;
    border-radius: 8px;
    cursor: text;
  }

  .task:hover {
    background: var(--hover);
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
    padding: 6px 8px 10px;
  }

  .add {
    width: 44px;
    height: 44px;
    border: none;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    font-size: 26px;
    line-height: 1;
    cursor: pointer;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.18);
  }

  .add:disabled {
    opacity: 0.5;
  }
</style>
