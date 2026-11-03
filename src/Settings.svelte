<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { invoke } from "@tauri-apps/api/core";
  import { getDb } from "./lib/db";
  import {
    DEFAULT_SHORTCUT,
    comboFromEvent,
    formatCombo,
  } from "./lib/shortcut";

  let db = $state<Awaited<ReturnType<typeof getDb>> | null>(null);
  let shortcut = $state("");
  let recording = $state(false);
  let status = $state("");
  let statusKind: "ok" | "err" = $state("ok");

  async function persist(combo: string) {
    if (!db) return;
    await db.execute(
      `INSERT INTO settings (key, value) VALUES ($1, $2)
       ON CONFLICT(key) DO UPDATE SET value = $3`,
      ["capture_shortcut", combo, combo],
    );
  }

  async function apply(combo: string) {
    recording = false;
    status = "";
    try {
      shortcut = await invoke<string>("set_shortcut", { shortcut: combo });
      await persist(combo);
      status = `Сохранено: ${formatCombo(combo)}`;
      statusKind = "ok";
    } catch (e) {
      status = String(e);
      statusKind = "err";
    }
  }

  function onWindowKeydown(e: KeyboardEvent) {
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
    if (e.key === "Escape") getCurrentWindow().hide();
  }

  onMount(async () => {
    shortcut = await invoke<string>("get_shortcut");
    db = await getDb();
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
      </div>
      <div class="control">
        <button
          class="combo"
          class:recording
          onclick={() => {
            recording = !recording;
            status = "";
          }}
        >
          {recording ? "Нажмите комбинацию…" : formatCombo(shortcut)}
        </button>
        {#if !recording && shortcut !== DEFAULT_SHORTCUT}
          <button class="ghost" onclick={() => apply(DEFAULT_SHORTCUT)}>
            Сбросить
          </button>
        {/if}
      </div>
    </div>

    {#if status}
      <p class="status" class:err={statusKind === "err"}>{status}</p>
    {/if}
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
  }

  .status.err {
    color: #d24c3f;
  }

  .soon {
    margin-top: 18px;
    text-align: center;
    font-size: 11px;
    color: var(--muted);
  }
</style>
