<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { emit, listen } from "@tauri-apps/api/event";
  import { getDb } from "./lib/db";
  import { dayLabel } from "./lib/dates";
  import {
    insertComment,
    loadComments,
    loadRawTask,
  } from "./lib/tasks";

  type RawTask = {
    id: number;
    day: string;
    text: string;
    source_id: number | null;
    is_done: number;
  };
  type Comment = { id: number; text: string; created_at: string };

  const label = getCurrentWindow().label;
  const windowTaskId = Number(label.slice("task-".length));

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let task = $state<RawTask | null>(null);
  let originalMissing = $state(false);
  let comments = $state<Comment[]>([]);
  let draft = $state("");
  let saving = $state(false);
  let error = $state("");
  let composer: HTMLTextAreaElement | null = $state(null);

  async function loadAll() {
    if (!db) return;
    try {
      const own = await loadRawTask(db, windowTaskId);
      let resolved = own;
      if (own?.source_id != null) {
        resolved = await loadRawTask(db, own.source_id);
        if (!resolved) {
          originalMissing = true;
          task = null;
          comments = [];
          error = "";
          return;
        }
      }
      originalMissing = false;
      task = resolved;
      comments = resolved ? await loadComments(db, resolved.id) : [];
      error = "";
    } catch (e) {
      error = `Не удалось загрузить задачу: ${String(e)}`;
    }
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
    if (saving) return;
    const text = draft.trim();
    if (!db || !task || !text) {
      draft = "";
      return;
    }
    saving = true;
    try {
      await insertComment(db, task.id, text);
      draft = "";
      await loadAll();
      emit("tasks-changed");
    } catch (e) {
      error = `Не удалось добавить комментарий: ${String(e)}`;
    } finally {
      saving = false;
    }
  }

  function onDraftKeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submitComment();
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") void getCurrentWindow().close();
  }

  function focus(el: HTMLTextAreaElement) {
    el.focus();
  }

  onMount(() => {
    getDb()
      .then(async (loaded) => {
        db = loaded;
        await loadAll();
      })
      .catch((e) => {
        error = `Не удалось открыть базу данных: ${String(e)}`;
      });

    const unlistenTasks = listen("tasks-changed", () => {
      void loadAll();
    });

    const focusComposer = () => composer?.focus();
    window.addEventListener("focus", focusComposer);

    return () => {
      window.removeEventListener("focus", focusComposer);
      void unlistenTasks.then((f) => f());
    };
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main class="viewer">
  {#if error}
    <p class="banner" role="alert">{error}</p>
  {/if}

  {#if originalMissing}
    <p class="none">Оригинал задачи не найден</p>
  {:else if task}
    <header class="head">
      <div class="date">{dayLabel(task.day)}</div>
      {#if task.is_done}
        <span class="done">выполнена</span>
      {/if}
    </header>

    <article class="task-text">{task.text}</article>

    <section class="feed" aria-label="Комментарии">
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
        bind:this={composer}
        rows="2"
        aria-label="Новый комментарий"
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

  .banner {
    margin: 10px 14px 0;
    padding: 6px 10px;
    border-radius: 7px;
    background: color-mix(in srgb, #d24c3f 14%, var(--bg));
    color: #d24c3f;
    font-size: 12px;
    line-height: 1.35;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
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

  .done {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--success, #34c759);
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
