<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { emit } from "@tauri-apps/api/event";
  import { getDb } from "./lib/db";
  import { dayLabel } from "./lib/dates";

  type Task = { id: number; day: string; text: string; source_id: number | null };
  type Comment = { id: number; text: string; created_at: string };

  const label = getCurrentWindow().label;
  const windowTaskId = Number(label.slice("task-".length));

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let task = $state<Task | null>(null);
  let originalMissing = $state(false);
  let comments = $state<Comment[]>([]);
  let draft = $state("");

  async function loadAll() {
    if (!db) return;
    let rows = await db.select<Task[]>(
      `SELECT id, day, text, source_id FROM tasks WHERE id = $1`,
      [windowTaskId],
    );
    let t = rows[0] ?? null;
    if (t?.source_id != null) {
      rows = await db.select<Task[]>(
        `SELECT id, day, text, source_id FROM tasks WHERE id = $1`,
        [t.source_id],
      );
      if (!rows[0]) {
        originalMissing = true;
        task = null;
        comments = [];
        return;
      }
      t = rows[0];
    }
    task = t;
    comments = t
      ? await db.select<Comment[]>(
          `SELECT id, text, created_at FROM comments
           WHERE task_id = $1 ORDER BY created_at`,
          [t.id],
        )
      : [];
  }

  function timeOf(iso: string): string {
    return new Date(iso).toLocaleString("ru-RU", {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  async function submitComment() {
    const text = draft.trim();
    if (!db || !task || !text) {
      draft = "";
      return;
    }
    await db.execute(
      `INSERT INTO comments (task_id, text, created_at) VALUES ($1, $2, $3)`,
      [task.id, text, new Date().toISOString()],
    );
    draft = "";
    await loadAll();
    emit("tasks-changed");
  }

  function onDraftKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submitComment();
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") getCurrentWindow().hide();
  }

  function focus(el: HTMLTextAreaElement) {
    el.focus();
  }

  onMount(() => {
    getDb().then(async (loaded) => {
      db = loaded;
      await loadAll();
    });
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main class="viewer">
  {#if originalMissing}
    <p class="none">Оригинал задачи не найден</p>
  {:else if task}
    <header class="head">
      <div class="date">{dayLabel(task.day)}</div>
    </header>

    <article class="task-text">{task.text}</article>

    <section class="feed">
      {#each comments as c}
        <div class="comment">
          <div class="when">{timeOf(c.created_at)}</div>
          <div class="body">{c.text}</div>
        </div>
      {/each}
      {#if comments.length === 0}
        <p class="none">Комментариев пока нет</p>
      {/if}
    </section>
  {:else if db}
    <p class="none">Задача не найдена</p>
  {/if}

  {#if task}
    <footer class="composer">
      <textarea
        use:focus
        rows="2"
        placeholder="Комментарий… (Enter — отправить, Shift+Enter — перенос)"
        bind:value={draft}
        onkeydown={onDraftKeydown}
      ></textarea>
    </footer>
  {/if}
</main>

<style>
  .viewer {
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
    padding: 14px 18px 4px;
    flex-shrink: 0;
  }

  .date {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .task-text {
    padding: 4px 18px 12px;
    font-size: 16px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
    border-bottom: 1px solid var(--hover);
    flex-shrink: 0;
  }

  .feed {
    flex: 1;
    overflow-y: auto;
    padding: 10px 18px;
  }

  .comment {
    padding: 8px 0;
  }

  .when {
    font-size: 11px;
    color: var(--muted);
    margin-bottom: 2px;
    user-select: none;
  }

  .body {
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  .none {
    text-align: center;
    color: var(--muted);
    margin-top: 30px;
  }

  .composer {
    padding: 10px 14px 14px;
    flex-shrink: 0;
  }

  .composer textarea {
    box-sizing: border-box;
    width: 100%;
    padding: 8px 10px;
    border: none;
    border-radius: 8px;
    background: var(--editor-bg);
    color: var(--ink);
    font: inherit;
    resize: none;
    outline: none;
    box-shadow: 0 0 0 1px var(--hover);
  }

  .composer textarea:focus {
    box-shadow: 0 0 0 1.5px var(--accent);
  }
</style>
