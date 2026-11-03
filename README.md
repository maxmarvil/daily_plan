# Daily Plan

Компактная записная книжка для быстрых задач на день. Концепция — в [CONCEPT.md](./CONCEPT.md).

Стек: **Tauri v2 + Rust**, фронтенд **Svelte 5 + Vite**, БД **SQLite** (`tauri-plugin-sql`).

## Запуск (dev)

```bash
npm install
npm run tauri dev
```

## Управление

- `Cmd+Shift+D` (глобально, по умолчанию) — показать/скрыть quick-capture окно.
  Комбинация настраивается в окне настроек (шестерёнка в шапке capture-окна,
  Esc — закрыть). Сохранённая комбинация применяется при каждом запуске.
- Кнопка `+` — новая задача текущего дня (Enter — сохранить, Shift+Enter — перенос строки).
- Клик по задаче — inline-редактирование.
- `Esc` — закрыть редактор, затем скрыть окно.
- Иконка календаря в шапке — окно календаря (режимы «Год»/«Месяц»); из
  «Месяца» — комментарии (💬) и перенос задач (↪).

Окно поверх всех (`alwaysOnTop`), при старте скрыто, приложение работает в режиме
агента (без иконки в Dock, `ActivationPolicy::Accessory`).

## Структура

- `src/` — Svelte-фронтенд (capture, календарь, просмотр задачи, настройки).
- `src-tauri/` — Rust: миграции SQLite, глобальные шорткаты, политика окон.
- `src-tauri/migrations/` — схема БД: пары `NNNN_name.up.sql` / `.down.sql`.
- БД: `daily-plan.db` в `app_data_dir`, схема — `CONCEPT.md` / миграции.

## Миграции

`tauri-plugin-sql` выполняет только Up при старте приложения.

Новая миграция: создать пару файлов `0007_name.up.sql` / `0007_name.down.sql`
в `src-tauri/migrations/` и добавить строку в `MIGRATIONS` в
`src-tauri/src/lib.rs` (up встраивается `include_str!`).

Ручной откат down-скриптами (сначала делает backup БД):

```bash
scripts/rollback.sh 5        # откатить к версии 5
DB_PATH=/tmp/test.db scripts/rollback.sh 0   # тест на копии
```

## Сборка релиза

Иконки пока заглушены (`bundle.icon: []`). Перед `npm run tauri build` сгенерировать:

```bash
npm run tauri icon path/to/1024.png
```
