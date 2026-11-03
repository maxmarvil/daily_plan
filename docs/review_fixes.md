# Исправление замечаний ревью — 2026-11-03

Отчёт о правках по [docs/code_review.md](./code_review.md). Версия проекта поднята до **0.2.0**.

## Итог

| Severity | Всего | Исправлено | Отложено/принято |
|---|---|---|---|
| HIGH | 2 | 2 | — |
| MEDIUM | 13 | 13 | — |
| LOW/NIT | 37 | 32 | 5 (см. «Отложено») |

Все high и medium закрыты, проверки зелёные: `npm test` (25 тестов), `npm run check`
(svelte-check: 0 ошибок, 0 предупреждений), `npm run build`, `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test` (5 тестов).

## Что исправлено

### HIGH

- **H1 — main-окно больше нельзя потерять.** `"closable": false` в
  `tauri.conf.json`; `on_window_event` перехватывает `CloseRequested` →
  `prevent_close()` + `hide()`. Кнопка закрытия у main отключена, а любое
  `CloseRequested` (если система его всё же инициирует) скрывает окно; все пути
  показа работают с существующим окном, а `open_window` переиспользует окно при
  гонке создания.
- **H2 — capture синхронизирован и не пишет «во вчера».** App слушает
  `tasks-changed`; на `capture-shown`/`new-task-shortcut` перепроверяется
  `todayStr()` и делается `refresh()`; `commitDraft` берёт дату на момент
  сохранения; `doTransfer` эмитит `tasks-changed`; редактор/черновик сбрасываются
  при смене суток.

### MEDIUM

- **M1 — переносы в capture с предпросмотром.** Общий слой запросов
  `src/lib/tasks.ts`: `LEFT JOIN` оригинала, `substr` 40 символов, схлопнутые
  переводы строк, счётчики комментариев/переносов, `COALESCE(s.is_done, t.is_done)`.
- **M2 — канонические шорткаты.** `src/lib/shortcut.ts`: `normalizeCombo`,
  `combosEqual`, `formatComboParts` понимают и `Command+Shift+D`, и
  `shift+super+KeyD` из Rust; настройки и применение сохранённых комбинаций
  сравнивают нормализованные значения. Добавлены тесты (17 в `shortcut.spec.ts`).
- **M3 — done принадлежит оригиналу.** Запросы показывают done оригинала,
  `setTaskDone` обновляет `source_id ?? id`; done разрешён любому дню. Описано в
  CONTEXT/CONCEPT/design_review.
- **M4 — гонки сохранения.** `saving` guard в `commitDraft`/`saveEdit`/`submitComment`,
  захват текущего `editing` перед сбросом, `pendingDone` против двойного клика.
- **M5 — видимые ошибки БД.** Баннер ошибки во всех окнах, `.catch` на `getDb()`,
  ошибки запросов не превращаются в пустой список.
- **M6 — перенос в другой месяц.** Пикер листает месяцы (`‹`/`›`), уже
  перенесённые дни недоступны, после переноса календарь переходит на месяц цели.
- **M7 — честное состояние шортката.** В Rust добавлен флаг `registered`:
  `set_*` не возвращают `Ok`, если комбинация не зарегистрирована; конфликт-чек
  учитывает только реально занятые комбинации. Команды `shortcut_registered` /
  `new_task_shortcut_registered` позволяют Settings показать предупреждение,
  если комбинацию заняла другая программа.
- **M8 — CSP.** Добавлен `connect-src ipc: http://ipc.localhost` — release-сборка
  больше не уходит в fallback через ошибку CSP.
- **M9 — инварианты в схеме.** Миграция `0008`: `UNIQUE(day, position)` и
  триггеры на формат `day`, text/source-правила, запрет self-source и
  переноса-переноса, календарную валидность `day` (`date(day) = day`),
  неизменяемость `day`/`created_at`/`source_id`, `is_done ∈ {0,1}`, комментарии
  только к оригиналам, запрет `DELETE`.
  В `tasks.ts` — `executeWithRetry` на коллизию `position`.
- **M10 — бэкапы и откат.** `scripts/rollback.sh` переписан: проверка закрытого
  приложения, preflight, `sqlite3 .backup` (WAL-safe) + `integrity_check`,
  откат каждой миграции в отдельной транзакции (при сбое посередине — частичный
  откат, восстановление из бэкапа). Ежедневный авто-бэкап: Rust-команды
  `prepare_backup` / `prune_backups` + `VACUUM INTO` из capture-окна, хранятся
  7 копий.
  README-раздел «Данные и бэкап» с предупреждениями.
- **M11 — документация синхронизирована.** Обновлены CONCEPT/CONTEXT/README/
  AGENTS/design_review: сетка 3×4, done, `settings`, второй шорткат, миграция
  0008, `$N`-параметры sqlx, «sqlx Pool» вместо «одно соединение», команды и CI.
- **M12 — a11y.** Строки задач — настоящие `<button>` (без вложенной
  интерактивности), `:focus-visible` для действий, `aria-label` у ячеек года и
  кнопок, `aria-pressed` у режимов/рекордеров, `aria-live` у статуса,
  `role="alert"` у ошибок, тултип года показывается и по фокусу.
- **M13 — typecheck/CI.** `svelte-check` + `npm run check`, `src/vite-env.d.ts`,
  `svelte.config.js`, `.github/workflows/ci.yml` (frontend + rust),
  `rust:lint`; TypeScript 6.x (`^6.0.3`).

### LOW / NIT

Закрыты: L1 (task-окна закрываются по Esc), L3 (бейдж/окно из capture),
L4 (индикатор переносов у оригинала), L5 (тултип очищается при скрытии),
L6 (heat/занятые дни в пикере), L7 (`transferring` сбрасывается),
L8 (`QueryResult.rowsAffected`), L9–L10 (loading/ticket-guard), L11 (нумерация
по `position`), L12 (трей эмитит `new-task-shortcut`), L14 (`.center()`),
L15 (взаимоисключающие рекордеры), L16 (комментарий в `db.ts`), L17 (down убран
из `MIGRATIONS`; down читает rollback.sh), L18 (`user-select: text` для текста
задач), L19 (модификатор обязателен в Rust), L20 (убраны `plugin-opener`,
JS `plugin-global-shortcut`, `serde` и лишние permissions; `serde_json` оставлен —
он нужен `tauri::generate_context!` — проверено: без него `cargo check` падает),
L21 (гонка открытия окон), L22 (статус сохранения комбинации), L23 (TaskView
слушает `tasks-changed`), L24 (диапазон вместо `substr(day,1,4)`), L25 (порядок
дня `DESC` задокументирован), L26 (переводы строк в предпросмотре),
L28 (`aria-live`, русские сообщения), L29 (`Default` используется, `expect` на
константах, Rust-тесты), L30 (общий `swap_global_shortcut`), L31 (лог в
`app_config_dir/daily-plan.log`, не только stderr), L32 (иконки сгенерированы,
`bundle.icon` заполнен), L33 (`windows_subsystem` убран, `rust-version`, `.editorconfig`),
L35 (README), L37 (`cargo fmt`/clippy).

## Отложено / принято осознанно

- **L2 (single-instance)** — нужен `tauri-plugin-single-instance`; в текущем
  окружении нет доступа к cargo-регистри (загрузка новых crates блокируется),
  поэтому плагин не добавлен. Задокументировано как известное ограничение;
  рекомендуется добавить вместе с `tauri-plugin-log` при следующем dependency-апдейте.
- **L13 (mono tray icon)** — требует включения `image-png` фичи Tauri и
  отдельного template-asset; отложено, чтобы не тянуть `image` crate. Сейчас
  в menu bar цветная иконка-заглушка.
- **L36 (roving tabindex в сетке года)** — nit; 365 ячеек остаются в обычном
  tab-порядке (у каждой есть `aria-label`).
- **L27 (Esc и черновик)** — поведение оставлено осознанно: Esc отменяет
  редактирование, скрытие окна шорткатом сохраняет черновик; зафиксировано в
  README (раздел «Управление»).
- **L34 (git)** — коммит не делался: в рабочем дереве смешаны незакоммиченные
  пользовательские WIP-изменения (done, второй шорткат) и эти правки. Нужен
  один коммит «0.2.0», когда пользователь будет готов; `git add docs/` обязателен
  (docs/ до сих пор untracked).
- **Подзапрос COUNT комментариев на строку месяца** — оставлен: индекс
  `idx_comments_task` есть, объём данных однопользовательский.

## Чем проверено

| Проверка | Результат |
|---|---|
| `npm test` | ✅ 25 тестов в 2 файлах (`dates`, `shortcut`) |
| `npm run check` (svelte-check + TS 6.x) | ✅ 0 ошибок, 0 предупреждений |
| `npm run build` | ✅ vite build (129 модулей) |
| `npm ci --dry-run` | ✅ lock согласован |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets -- -D warnings` | ✅ без предупреждений |
| `cargo test` | ✅ 5 тестов (миграции, дефолтные шорткаты, round-trip display, требование ⌘/⌃/⌥, дата бэкапа) |
| Миграции 1…8 на свежей БД | ✅ применяются по порядку |
| Миграция `0008` на копии живой БД | ✅ применяется, данные целы |
| Отклонение невалидных данных (16 кейсов, включая `2026-11-31` и `2026-13-45`) | ✅ триггеры: формат day, пустой/непустой text, self-source, перенос-переноса, дубль position, DELETE, комментарий к переносу, неизменяемость, is_done=2 |
| SQL из `src/lib/tasks.ts` на копии живой БД | ✅ select/insert/transfer-guard/update done оригинала |
| `scripts/rollback.sh` 8→6 | ✅ бэкап + integrity ok, триггеры 0008 сняты, версия 6 |
| `VACUUM INTO ?` с bind-параметром | ✅ на sqlite3 (путь с пробелами в `/Application Support/` работает) |

## Требует ручного прогона (GUI не запускался)

- Закрытие main-окна (кнопка отключена, `CloseRequested` перехватывается) и
  возврат capture шорткатом/треем; поведение ⌘W зависит от macOS — проверить.
- Два глобальных шортката и их отображение чипами в настройках.
- Синхронизация capture ↔ календарь (done, комментарии, перенос в сегодня).
- Кросс-месячный перенос и переход на месяц цели.
- Появление `backups/daily-plan-YYYY-MM-DD.db` после запуска (VACUUM INTO через
  `tauri-plugin-sql`; сам SQL проверен отдельно).
- release-сборка с новыми иконками и CSP `connect-src ipc:`.

## Изменённые/новые файлы

- Rust: `src-tauri/src/lib.rs` (переработан), `Cargo.toml` (0.2.0, `rust-version`),
  `src-tauri/src/main.rs`, `tauri.conf.json` (closable, CSP, иконки, 0.2.0),
  `capabilities/default.json` (least privilege), `migrations/0008_data_integrity.{up,down}.sql`.
- Frontend: `src/App.svelte`, `src/Calendar.svelte`, `src/TaskView.svelte`,
  `src/Settings.svelte`, `src/lib/tasks.ts` (new), `src/lib/settings.ts` (new),
  `src/lib/shortcut.ts`, `src/lib/dates.ts`, `src/lib/db.ts`, спек-файлы.
- Инфраструктура: `scripts/rollback.sh`, `.github/workflows/ci.yml` (new),
  `svelte.config.js` (new), `src/vite-env.d.ts` (new), `.editorconfig` (new),
  `CHANGELOG.md` (new), `package.json` / lock (scripts, svelte-check, TS 6.x,
  удалён opener), `src-tauri/icons/*` (сгенерированы), `.gitignore`.
- Документация: `CONCEPT.md`, `CONTEXT.md`, `README.md`, `AGENTS.md`,
  `docs/design_review.md`, этот файл.


## Дополнительно после независимой проверки

Независимые верификаторы (код, данные/конфиги/доки) не нашли blocker/high-регрессий.
По их замечаниям дополнительно исправлено:

- **Потеря inline-правки при показе capture** (medium в их отчёте): `onShown`
  теперь сначала сохраняет открытый редактор (`saveEdit`), и только потом
  переключается на поле новой задачи; добавлен фокус в поле при повторном
  показе (`tick` + `bind:this`).
- **Шорткаты**: при совпадении комбинации с сохранённой, но незарегистрированной
  (`registered == false`) сеттеры теперь только регистрируют её, не снимая
  свежую регистрацию; добавлены команды `shortcut_registered` /
  `new_task_shortcut_registered`, и Settings показывает предупреждение, если
  комбинацию заняла другая программа; `parse_shortcut` больше не принимает
  Shift-only комбинации (нужен ⌘/⌃/⌥).
- **Calendar**: обновление списка и очистка тултипа по focus/blur окна (на случай
  скрытия/показа без `tasks-changed`); удалён неиспользуемый `maxCount`.
- **TaskView**: фокус в поле комментария при возврате в окно.
- **Документация**: осторожные формулировки про ⌘W (кнопка закрытия отключена,
  `CloseRequested` перехватывается), `COUNT(*)` вместо `EXISTS`, TypeScript 6.x
  (`^6.0.3`), «откат каждой миграции в отдельной транзакции», «бэкап при запуске,
  если за сегодня его ещё нет», добавлена календарная валидность `day` в M9.
- **`serde`** удалён из `Cargo.toml` (не использовался); `serde_json` оставлен —
  без него `cargo check` падает на `tauri::generate_context!`.

Повторные проверки после этих правок: `npm test` 25/25, `npm run check` 0/0,
`npm run build` ✅, `cargo fmt --check` ✅, `cargo clippy -D warnings` ✅,
`cargo test` 5/5.
