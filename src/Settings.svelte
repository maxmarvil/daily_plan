<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { getDb } from "./lib/db";
  import {
    SETTING_CAPTURE_SHORTCUT,
    SETTING_NEW_TASK_SHORTCUT,
    setSetting,
  } from "./lib/settings";
  import {
    DEFAULT_SHORTCUT,
    DEFAULT_NEW_TASK_SHORTCUT,
    comboFromEvent,
    combosEqual,
    formatCombo,
    formatComboParts,
  } from "./lib/shortcut";

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let shortcut = $state("");
  let recording = $state(false);
  let newTaskShortcut = $state("");
  let recordingNewTask = $state(false);
  let status = $state("");
  let statusKind: "ok" | "err" | "warn" = $state("ok");
  let captureRegistered = $state(true);
  let newTaskRegistered = $state(true);

  function setStatus(kind: "ok" | "err" | "warn", text: string) {
    statusKind = kind;
    status = text;
  }

  async function persist(key: string, value: string): Promise<string | null> {
    if (!db) return "База данных не загружена — комбинация не сохранена";
    try {
      await setSetting(db, key, value);
      return null;
    } catch (e) {
      return `Комбинация применена, но не сохранена: ${String(e)}`;
    }
  }

  async function apply(combo: string) {
    recording = false;
    status = "";
    try {
      const applied = await invoke<string>("set_shortcut", { shortcut: combo });
      shortcut = applied;
      captureRegistered = true;
      const warning = await persist(SETTING_CAPTURE_SHORTCUT, applied);
      if (warning) {
        setStatus("warn", warning);
      } else {
        setStatus("ok", `Сохранено: ${formatCombo(applied)}`);
      }
    } catch (e) {
      setStatus("err", String(e));
    }
  }

  async function applyNewTask(combo: string) {
    recordingNewTask = false;
    status = "";
    try {
      const applied = await invoke<string>("set_new_task_shortcut", {
        shortcut: combo,
      });
      newTaskShortcut = applied;
      newTaskRegistered = true;
      const warning = await persist(SETTING_NEW_TASK_SHORTCUT, applied);
      if (warning) {
        setStatus("warn", warning);
      } else {
        setStatus("ok", `Сохранено: ${formatCombo(applied)}`);
      }
    } catch (e) {
      setStatus("err", String(e));
    }
  }

  function toggleRecording() {
    recordingNewTask = false;
    recording = !recording;
    status = "";
  }

  function toggleRecordingNewTask() {
    recording = false;
    recordingNewTask = !recordingNewTask;
    status = "";
  }

  function onWindowKeydown(e: KeyboardEvent) {
    // взаимоисключающие рекордеры: активен всегда только один
    if (recording) {
      if (e.key === "Escape") {
        e.preventDefault();
        recording = false;
        status = "";
        return;
      }
      const combo = comboFromEvent(e);
      if (combo) {
        e.preventDefault();
        void apply(combo);
      }
      return;
    }
    if (recordingNewTask) {
      if (e.key === "Escape") {
        e.preventDefault();
        recordingNewTask = false;
        status = "";
        return;
      }
      const combo = comboFromEvent(e);
      if (combo) {
        e.preventDefault();
        void applyNewTask(combo);
      }
      return;
    }
    if (e.key === "Escape") void getCurrentWindow().hide();
  }

  onMount(() => {
    void (async () => {
      try {
        shortcut = await invoke<string>("get_shortcut");
        newTaskShortcut = await invoke<string>("get_new_task_shortcut");
        captureRegistered = await invoke<boolean>("shortcut_registered");
        newTaskRegistered = await invoke<boolean>("new_task_shortcut_registered");
      } catch (e) {
        setStatus("err", `Не удалось прочитать комбинации: ${String(e)}`);
      }
      try {
        db = await getDb();
      } catch (e) {
        setStatus("err", `Не удалось открыть базу данных: ${String(e)}`);
      }
    })();
  });
</script>

<svelte:window onkeydown={onWindowKeydown} />

<main class="settings">
  <h1>Настройки</h1>

  <section class="group">
    <h2>Горячие клавиши</h2>

    <div class="row">
      <div class="field">
        <div class="label">Вызов окна задач</div>
        <div class="hint">
          Работает глобально, пока приложение запущено. Нужен хотя бы один
          модификатор (⌘, ⌃, ⌥) и клавиша.
        </div>
        {#if !captureRegistered}
          <div class="hint warn">
            Комбинация не зарегистрирована (возможно, её заняла другая
            программа) — задайте другую.
          </div>
        {/if}
      </div>
      <div class="control">
        <button
          class="combo"
          class:recording
          aria-pressed={recording}
          onclick={toggleRecording}
        >
          {#if recording}
            Нажмите комбинацию…
          {:else}
            {#each formatComboParts(shortcut) as part}<span class="key"
                >{part}</span
              >{/each}
          {/if}
        </button>
        {#if !recording && !combosEqual(shortcut, DEFAULT_SHORTCUT)}
          <button class="ghost" onclick={() => apply(DEFAULT_SHORTCUT)}>
            Сбросить
          </button>
        {/if}
      </div>
    </div>

    <div class="row">
      <div class="field">
        <div class="label">Новая задача</div>
        <div class="hint">
          Показывает окно и сразу активирует поле ввода новой задачи.
        </div>
        {#if !newTaskRegistered}
          <div class="hint warn">
            Комбинация не зарегистрирована (возможно, её заняла другая
            программа) — задайте другую.
          </div>
        {/if}
      </div>
      <div class="control">
        <button
          class="combo"
          class:recording={recordingNewTask}
          aria-pressed={recordingNewTask}
          onclick={toggleRecordingNewTask}
        >
          {#if recordingNewTask}
            Нажмите комбинацию…
          {:else}
            {#each formatComboParts(newTaskShortcut) as part}<span class="key"
                >{part}</span
              >{/each}
          {/if}
        </button>
        {#if !recordingNewTask && !combosEqual(newTaskShortcut, DEFAULT_NEW_TASK_SHORTCUT)}
          <button class="ghost" onclick={() => applyNewTask(DEFAULT_NEW_TASK_SHORTCUT)}>
            Сбросить
          </button>
        {/if}
      </div>
    </div>

    <p
      class="status"
      class:err={statusKind === "err"}
      class:warn={statusKind === "warn"}
      role="status"
      aria-live="polite"
    >{status}</p>
  </section>

  <p class="soon">Другие настройки появятся здесь</p>
</main>

<style>
  .settings {
    height: 100vh;
    overflow-y: auto;
    padding: 18px 22px 26px;
    font-family:
      -apple-system, "SF Pro Text", "Helvetica Neue", sans-serif;
    font-size: 13px;
    color: var(--ink);
    background: var(--bg);
    box-sizing: border-box;
  }

  h1 {
    margin: 0 0 16px;
    font-size: 17px;
    font-weight: 700;
  }

  h2 {
    margin: 0 0 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .group {
    border: 1px solid var(--hover);
    border-radius: 10px;
    padding: 12px 14px 14px;
    background: var(--editor-bg);
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 0 6px;
  }

  .field {
    flex: 1;
  }

  .label {
    font-size: 13px;
    font-weight: 600;
  }

  .hint {
    margin-top: 3px;
    font-size: 11px;
    line-height: 1.4;
    color: var(--muted);
  }

  .hint.warn {
    color: #b8860b;
  }

  .control {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .combo {
    min-width: 120px;
    padding: 7px 12px;
    border: 1px solid var(--hover);
    border-radius: 8px;
    background: var(--bg);
    color: var(--ink);
    font-size: 13px;
    cursor: pointer;
  }

  .combo:hover {
    border-color: var(--accent);
  }

  .combo.recording {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px
      color-mix(in srgb, var(--accent) 30%, transparent);
    color: var(--accent);
  }

  .key {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 22px;
    padding: 2px 5px;
    border-radius: 5px;
    background: var(--hover);
    color: var(--ink);
    font-size: 12px;
    font-weight: 600;
  }

  .combo:hover .key {
    background: color-mix(in srgb, var(--accent) 12%, var(--hover));
  }

  .ghost {
    border: none;
    background: none;
    color: var(--muted);
    font-size: 12px;
    cursor: pointer;
    padding: 6px 8px;
    border-radius: 6px;
  }

  .ghost:hover {
    background: var(--hover);
    color: var(--ink);
  }

  .status {
    margin: 6px 0 0;
    font-size: 12px;
    color: var(--muted);
    min-height: 1em;
  }

  .status.err {
    color: #d24c3f;
  }

  .status.warn {
    color: #b8860b;
  }

  .soon {
    margin-top: 18px;
    text-align: center;
    font-size: 11px;
    color: var(--muted);
  }

  .combo:focus-visible,
  .ghost:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }
</style>
