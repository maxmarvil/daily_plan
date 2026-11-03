# Daily Plan

Компактная записная книжка для быстрых задач на день. Концепция — в [CONCEPT.md](./CONCEPT.md).

Стек: **Tauri v2 + Rust**, фронтенд **Svelte 5 + Vite**, БД **SQLite** (`tauri-plugin-sql`).

## Запуск (dev)

```bash
npm install
npm run tauri dev
```

## Управление

- `Cmd+Shift+D` (глобально) — показать/скрыть quick-capture окно.
- Кнопка `+` — новая задача текущего дня (Enter — сохранить, Shift+Enter — перенос строки).
- Клик по задаче — inline-редактирование.
- `Esc` — закрыть редактор, затем скрыть окно.

Окно поверх всех (`alwaysOnTop`), при старте скрыто, приложение работает в режиме
агента (без иконки в Dock, `ActivationPolicy::Accessory`).

## Структура

- `src/` — Svelte-фронтенд (quick-capture).
- `src-tauri/` — Rust: миграции SQLite, глобальный шорткат, политика окна.
- БД: `daily-plan.db` в `app_data_dir`, схема — `CONCEPT.md` / `src-tauri/src/lib.rs`.

## Сборка релиза

Иконки пока заглушены (`bundle.icon: []`). Перед `npm run tauri build` сгенерировать:

```bash
npm run tauri icon path/to/1024.png
```
