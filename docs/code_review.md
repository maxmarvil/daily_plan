# Code review — Daily Plan

> **Статус:** замечания исправлены в версии 0.2.0 — что именно закрыто,
> отложено и как проверено, см. [docs/review_fixes.md](./review_fixes.md).

**Дата:** 2026-11-03
**Область:** рабочее дерево `main` @ `8912572` + незакоммиченные изменения (done-состояние, миграция 0007, второй глобальный шорткат, `docs/design_review.md`).
**Объём:** ~3 080 строк (Svelte 1 777, TS 332, Rust 462, SQL 40, документация 390).
**Метод:** чтение всех файлов проекта; 3 независимых ревьюера (Rust/Tauri, фронтенд, данные/документация); Lead свёл и перепроверил ключевые находки по исходникам закреплённых версий зависимостей (`tauri 2.12.5`, `global-shortcut 2.4.1`, `global-hotkey 0.8.0`, `tauri-plugin-sql 2.6.0`, `sqlx 0.8.6`, `tauri-codegen 2.7.4`), по коду и на копии живой БД.

**Ограничения:** GUI-приложение не запускалось — сценарии с окнами/треем/шорткатами проверены по коду; release-сборка не собиралась; визуальная проверка не проводилась. Пункты, требующие ручного прогона, помечены «требует проверки».

---

## Вердикт

Для MVP проект в хорошем состоянии: маленький и понятный, сборки зелёные, SQL полностью параметризован, миграции с чексуммами целы, живая БД проходит integrity-проверки. **Блокеров нет.**

Главные риски — не в компиляции, а в **краевых сценариях и синхронизации окон**: primary-окно можно навсегда закрыть по ⌘W, capture-окно не обновляется после изменений в календаре/окне задачи и не перепроверяет дату при показе, а незакоммиченный рефакторинг шорткатов сломал отображение комбинаций в настройках. Плюс системный долг: нет typecheck/lint/CI, инварианты данных не защищены схемой, документация разошлась с кодом.

**Найдено:** 2 high, 13 medium, 37 low/nit.

---

## Что проверено

| Проверка | Результат |
|---|---|
| `npm test` | ✅ 13/13 (2 файла: `dates.spec.ts`, `shortcut.spec.ts`) |
| `npm run build` | ✅ vite build, 127 модулей, 79.2 kB JS / 11.6 kB CSS |
| `cargo check` | ✅ без ошибок и предупреждений |
| `cargo fmt --check` | ❌ расхождения: импорты `lib.rs:1-9`, `get_*_shortcut` `152-164`, сигнатура `shortcut_in_use:167` |
| `cargo clippy --all-targets` | ⚠️ 12 предупреждений (в основном `clone_on_copy` для `Shortcut`; 2 лишние конверсии) |
| `npx tsc --noEmit` | ❌ `src/main.ts:7: TS2882` (импорт `./app.css`); `.svelte`-скрипты tsc не проверяет |
| Аудит копии живой БД | ✅ миграции 1–7 `success=1`, checksums = sha384 up-файлов; WAL; 0 дублей переносов, битых `source_id`, дублей `position` |
| Rollback/конкурентные вставки | ✅ на копиях БД: 7→0 + повторный прогон up даёт идентичную схему; 600 параллельных `INSERT … SELECT` — 0 дублей `position` |

Проверенные факты-«не-баги» (чтобы не переизобретать): `default_window_icon()` при пустом `bundle.icon` на macOS не равен `None` — codegen фолбэчится на `src-tauri/icons/icon.png`, поэтому tray создаётся; повтор `$1` в SQL корректен (sqlx биндит по номеру параметра, `$1` дважды = один параметр); FK в sqlx включены по умолчанию на каждом соединении; гонка «два окна → дубли `position`» не воспроизводится (атомарный `INSERT … SELECT` + сериализация писателей SQLite).

---

## HIGH

### H1. Capture-окно можно закрыть навсегда — вернуть его без перезапуска нельзя

- `src-tauri/tauri.conf.json:14-25` — у `main` не выставлен `"closable": false`; приложение не задаёт своё меню, поэтому Tauri ставит дефолтное macOS-меню (`tauri-2.12.5/src/app.rs:2410`, `src/menu/menu.rs:142,163`) с пунктом **Close Window ⌘W** (`muda` → `performClose:`). `decorations` по умолчанию `true`, т.е. есть и красная кнопка.
- Обработчика `CloseRequested` нет (`src-tauri/src/lib.rs:362-455`). После закрытия окно уничтожается.
- Все пути показа capture работают только с существующим окном: шорткаты `lib.rs:382-392`, трей `lib.rs:340-346` делают `get_webview_window("main")` → `None` → **молча ничего**. Трей «Новая задача» / повторный вызов шортката перестают работать; приложение при этом остаётся жить в трее (Accessory, без иконки в Dock). Спасает только «Выход» + перезапуск, набранный текст теряется.
- **Фикс:** `"closable": false` для `main` и/или `on_window_event(|window, event| if let WindowEvent::CloseRequested { api, .. } = event { api.prevent_close(); let _ = window.hide(); })`. Дополнительно — хелпер «показать main», который пересоздаёт окно, если его нет (защита от будущих сценариев destroy).

### H2. Capture-окно не синхронизировано и не перепроверяет дату

- `src/App.svelte:158-193`: окно слушает только `capture-shown`/`new-task-shortcut` (обе лишь ставят `adding = true`) и **не слушает `tasks-changed`**, хотя `Calendar.svelte:264-268` его слушает, а App сам его эмитит (`68,95,105`). Итог: пометил done/добавил комментарий/перенёс задачу на сегодня в календаре → скрыл и показал capture по шорткату → **список stale** (неверный `is_done`, перенесённой задачи нет).
- `src/Calendar.svelte:224-249` — `doTransfer` не эмитит `tasks-changed` (в отличие от `toggleDone:214-222`), поэтому о переносе не узнаёт даже календарь в другом окне.
- `src/App.svelte:167-186`: дата `day` обновляется только таймером раз в 30 с. В скрытом WKWebView (App Nap/троттлинг) и после сна таймер может не сработать вовремя; `capture-shown` дату не сверяет, `commitDraft` берёт `day` из состояния (`44-73`). Сценарий: утром нажал шорткат, сразу начал печатать → **задача может уйти во вчерашний день** (в первые 30 с после полуночи — тоже).
- Бонус: если окно скрыть шорткатом при открытом редакторе, `editing` не сбрасывается; на следующем показе `adding = true` → одновременно открыты редактор задачи и поле новой задачи (`253-261`, `290-299`).
- **Фикс:** в `capture-shown`/`new-task-shortcut`: `const t = todayStr(); if (t !== day) { day = t; editing = null; draft = ""; } adding = true; await refresh();`; подписаться на `tasks-changed` → `refresh()`; в `doTransfer` добавить `emit("tasks-changed")`; в `commitDraft` брать `todayStr()` на момент вставки.

---

## MEDIUM

### M1. Переносы в capture-окне рендерятся пустыми строками

`src/App.svelte:33-42` выбирает `text` как есть, без JOIN на оригинал. У записи-переноса `text = ''` (`Calendar.svelte:232-239`), предпросмотр строится только в запросе календаря (`Calendar.svelte:129-143`). Поэтому перенесённая на сегодня задача в главном окне выглядит как **пустая плашка с номером и оранжевым фоном** — задача визуально теряется. Фикс: тот же `LEFT JOIN tasks s ON s.id = t.source_id` + `substr(…,1,40)` / `truncated`, как в календаре; вынести запрос в общий helper.

### M2. Регрессия контракта канонической строки шортката (незакоммиченный рефакторинг)

Rust теперь отдаёт `Shortcut::to_string()` из `global-hotkey` = **`shift+super+KeyD`** (проверено запуском на собранном rlib), а `src/lib/shortcut.ts:4-25` и `Settings.svelte:35-37,49-50,129,159` ожидают `Command+Shift+D`/`CmdOrCtrl+Shift+D`.

Последствия (видны сразу, у пользователя в `settings` сейчас `Alt+Shift+N` и `Alt+Shift+T`):
- чипы комбинаций показывают сырые токены «shift alt KeyN» вместо `⌥ ⇧ N`;
- `shortcut !== DEFAULT_SHORTCUT` всегда true → кнопка «Сбросить» видна всегда;
- в статус «Сохранено: shift alt KeyN»;
- `App.svelte:135-152` на каждом старте видит «несовпадение» persisted-строки и canonical-ответа и вызывает `set_shortcut` (обычно no-op, но логика сравнения неверна);
- `Settings` сохраняет raw `combo`, а не возвращённый canonical.

Фикс: один канонический формат. Проще всего — Rust возвращает исходную строку ввода (`Shortcut` хранить вместе с canonical-строкой, `Mutex<(Shortcut, String)>`), либо фронт нормализует display через отдельную маппинг-функцию для `shift/super/control/alt/KeyX/DigitN`. Persist-ить возвращённое значение. Добавить тест round-trip (Rust-тест `parse → to_string`).

### M3. `is_done` у переносов и прошлых дней: модель не определена

- `Calendar.svelte:214-219`, `App.svelte:98-106` переключают `is_done` у конкретной строки. У записи-переноса своё состояние → одна и та же задача может быть «выполнена» в одном дне и «не выполнена» в другом. При этом комментарии и бейджи резолвятся к оригиналу (`COALESCE(source_id,id)`, `TaskView.svelte:22-48`) — модель ведёт себя непоследовательно.
- done доступен и для **прошлых** дней, хотя `CONTEXT.md:5,11-12` говорит «прошлые дни — только комментируются», а `update_at` меняется.
- В `CONCEPT.md`/`CONTEXT.md` состояния done нет вообще — оно описано только в `docs/design_review.md:53-64`. Heatmap года при этом считает выполненные задачи активностью.
- **Решение продуктовое:** либо done хранить/переключать у оригинала (`WHERE id = COALESCE(source_id, id)`), либо явно задокументировать per-link семантику; описать в CONTEXT/CONCEPT, разрешён ли done прошлым дням.

### M4. Гонки сохранения в capture-окне

- `App.svelte:84-96`: `saveEdit` после `await` безусловно ставит `editing = null` — если к этому моменту пользователь уже открыл другой редактор, он закроется (`blur` → `saveEdit` A, клик → `startEdit` B, завершение A закрывает B).
- `App.svelte:44-73,84-96`, `TaskView.svelte:59-72`: нет in-flight guard. Enter + клик мимо (blur) или повторный Enter в пределах `await` → второй INSERT (draft очищается только после await) → дубликат задачи/комментария. Двойной клик по done читает одинаковый `is_done` до перезагрузки → один переворот на два клика.
- **Фикс:** флаг `saving`/`pendingDone: Set<number>`, захват `const cur = editing` и сравнение перед сбросом, очистка draft до/вместе с блокировкой.

### M5. Ошибки БД не доходят до пользователя

`getDb()` без `.catch` во всех окнах (`App.svelte:159`, `Calendar.svelte:259`, `TaskView.svelte:90`, `Settings.svelte:91-95`) — при сбое загрузки БД/миграции окно просто пустое (capture — пустой список, календарь — пустая сетка), без сообщения и ретрая. Ошибки `select`/`execute` в `saveEdit`/`toggleDone`/`submitComment`/`toggleDone` не обрабатываются → unhandled rejection, действие «молча не сработало»; `commitDraft`/`doTransfer` хотя бы логируют в консоль. Плюс `rows.length === 0` (`Calendar.svelte:392-394`) при ошибке покажет ложное «В этом месяце задач нет». **Фикс:** состояние `error` + видимый баннер/toast, общий хелпер `runQuery` с try/catch и ретраем.

### M6. Перенос в другой месяц невозможен

`Calendar.svelte:193-199,373-389`: пикер переноса показывает только дни **просматриваемого** месяца (`monthDays` от `year/month`), а строка, которую переносим, видна только в своём месяце. Перенести задачу из ноября в декабрь через UI нельзя, хотя `CONCEPT.md` не ограничивает день B. Фикс: навигация по месяцам внутри пикера / ввод даты / «сегодня + следующий месяц» пресеты.

### M7. State шорткатов врёт при отказе регистрации дефолта

`lib.rs:396-435`: `.manage(ActiveShortcut(default))` выставляет «активные» значения **до** регистрации, а setup обновляет state только при успехе. Если дефолтный шорткат занят другой программой (сценарий, ради которого и открывается Settings), `get_shortcut` возвращает неработающую комбинацию, а `set_shortcut(default)` упирается в `current == new` и возвращает `Ok` **без регистрации** — пользователь видит «Сохранено», клавиша не работает; конфликт-проверка блокирует комбинацию, которую на деле никто не держит. Фикс: хранить факт регистрации (`Option<Shortcut>`/`AtomicBool`), early-return при `current == new` только если реально зарегистрировано.

### M8. CSP без `connect-src ipc: http://ipc.localhost`

`tauri.conf.json:28`: `default-src 'self'` без `connect-src`. IPC в Tauri идёт через `fetch("ipc://localhost/…")` (`tauri-2.12.5/scripts/ipc-protocol.js`); в release CSP блокирует custom-protocol fetch, JS один раз логирует `IPC custom protocol failed` и переключается на `postMessage`-fallback. Функционально не ломается, но каждый запуск даёт CSP-нарушение/шум в консоли и неочевидный путь. В dev CSP не применяется, поэтому не видно. Фикс: добавить `connect-src ipc: http://ipc.localhost` (как в примере из документации Tauri: https://v2.tauri.app/security/csp/).

### M9. Инварианты данных не защищены схемой

Схема (0001/0003/0005) гарантирует только `UNIQUE (source_id, day)` (частичный индекс) и FK. На копии БД проверено, что принимаются: `day = '03.11.2026'`, пустой `text` у обычной задачи, `source_id = id` (self-source), перенос с непустым `text`, `is_done = 2`, перенос-на-перенос; `DELETE` задачи каскадно стирает комментарии (`ON DELETE CASCADE`, 0003:4) — инвариант «задачи не удаляются» держится только соглашением. Правило «править текст можно только в свой день» вообще не enforced на бэкенде: все CRUD идут из JS через plugin-sql, Rust-команд для задач нет. `UNIQUE (day, position)` отсутствует, поэтому «номера стабильны» без гарантии (гонку на практике воспроизвести не удалось, но реальный источник дублей — повторная вставка в одном окне, см. M4).

Фикс (в порядке цены): `CHECK`-констрейнты (`day` glob, `source_id <> id`, `is_done IN (0,1)`, `text <> '' OR source_id IS NOT NULL`), `UNIQUE (day, position)` + retry по `SQLITE_BUSY`, триггеры/`BEFORE DELETE RAISE(ABORT)` для задач и «комментарий только к не-переносу»; либо осознанно записать в CONTEXT «инварианты на уровне UI» и защититься позже Rust-репозиторием (сам CONCEPT это предполагает, `CONCEPT.md:95-96`).

### M10. Бэкап и ручной откат небезопасны

- `scripts/rollback.sh:25-27`: «backup» через `cp` на WAL-БД. Пока соединение открыто, закоммиченные данные лежат в `-wal`; проверено: в `cp`-копии закоммиченная строка `settings` отсутствует. Нет проверки, что приложение закрыто. Фикс: `sqlite3 "$DB" ".backup '$BACKUP'"` (или `VACUUM INTO`), проверка `pgrep`/`-wal`, pre-flight наличие всех down-файлов до начала отката.
- Down-скрипты выполняются вне транзакции (`rollback.sh:29-35`) — сбой на середине оставляет частичный откат; `0007….down.sql` использует `DROP COLUMN` (SQLite ≥ 3.35, системная библиотека) — на старых macOS упадёт.
- `README.md:43-48` подаёт откат как штатную операцию, но откат до 5/6 удаляет `settings` (сохранённые шорткаты), до 0 — tasks/comments; после перезапуска plugin применит недостающие up-миграции заново.
- Нет автобэкапа при старте и проверки версии схемы/приложения: неудачная миграция оставит `success=0` (Dirty), а приложение завершится с `eprintln!` + `exit(1)` без подсказки восстановления (`lib.rs:451-455`).

### M11. Документация разошлась с кодом

- `CONCEPT.md:31,157`, `CONTEXT.md:43` — сетка года «4×3», фактически **3 колонки × 4 ряда** (`Calendar.svelte:493-499`, `docs/design_review.md:38`).
- `CONCEPT.md:99-126` — модель данных устарела: нет `settings` (0006), нет `tasks.is_done` (0007), у `text` есть `DEFAULT ''`.
- `CONCEPT.md:59-60,144-145` — «окно просмотра, если комментариев ≥ 1, по клику на бейдж»; фактически окно открывает клик по любой строке (`Calendar.svelte:184-191,333`), бейдж работает и при 0 (`352-361`).
- `CONCEPT.md:41` — «клик по ссылке открывает исходную задачу»; в режиме «Месяц» клик по переносу только переключает месяц (`Calendar.svelte:188-191`), окно — только в capture (`App.svelte:75-82`).
- `CONCEPT.md:46-51` — индикатор «у оригинала есть хотя бы один перенос» не реализован (в запросе календаря нет подсчёта переносов).
- `CONCEPT.md:57-58` — бейдж комментариев «у задачи»; в capture его нет.
- `README.md:14-24` — не документирован второй шорткат «Новая задача», done-кнопки, tray-меню, раздел «данные/бэкап»; `README.md:52` про иконки стоит уточнить (tray/окну иконка достаётся из `icons/icon.png`).
- `AGENTS.md:34-35` — нет `get_new_task_shortcut`/`set_new_task_shortcut`; `AGENTS.md:63` «Commands return `Result<String,String>`» — оконные команды возвращают `tauri::Result<()>` (`lib.rs:114,260,285`); `AGENTS.md:36-37` «one connection it keeps» неверно (см. L16); `AGENTS.md:55-56` правило про `$N` неверно для sqlx.
- `docs/design_review.md` и `docs/` вообще не в git.

### M12. Доступность (a11y)

- `App.svelte:263-286`, `Calendar.svelte:327-371`: `div role="button" tabindex="0"` содержит настоящие `<button>`; keydown родителя ловит Enter со вложенной кнопки (в детях нет `stopPropagation` на keydown) → Enter на кнопке done одновременно переключает done и открывает редактор/оригинал. Невалидная интерактивная вложенность.
- `App.svelte:405-425`, `Calendar.svelte:635-663,684-700`: действия показываются только на hover (`opacity:0`), с клавиатуры фокус уходит на невидимую кнопку; `:focus-visible` в проекте нет вовсе.
- `Calendar.svelte:303-313`: ячейки года без `aria-label` (только число), число задач доступно лишь mouse-tooltip → с клавиатуры/SR недоступно.
- Мелочи: сегменты без `aria-pressed`, статус Settings без `aria-live`, textarea без `aria-label`, `✕` без `aria-label`, 365 кнопок в обычном tab-порядке.

### M13. Нет typecheck/lint/CI; конфигурация форматирования расходится

- `package.json:6-12` — нет `svelte-check`/`typecheck`/`lint`; `tsc --noEmit` падает на импорте CSS, а `.svelte` не проверяются вообще (несмотря на `tsconfig.json:6`). Именно поэтому не пойманы `QueryResult === 0` (L8) и несовпадение типов шорткатов.
- Нет `svelte.config.js` (vite-plugin-svelte предупреждает), нет ESLint/Prettier, нет `.github`/CI, нет coverage. `cargo fmt --check` и `cargo clippy` не входят ни в одну проверку.

---

## LOW / NIT

| # | Место | Находка | Что сделать |
|---|---|---|---|
| L1 | `lib.rs:285-306`, `TaskView.svelte:81-83` | Окна `task-{id}` при Esc **скрываются**, не закрываются; скрытые webview копятся за сессию (память) | Закрывать окно (`close()`) или переиспользовать одно окно задачи с событием смены id |
| L2 | `Cargo.toml` | Нет `tauri-plugin-single-instance`: второй экземпляр = второй процесс/трей, конфликт шорткатов, два писателя в SQLite | Добавить плагин + фокус существующего окна |
| L3 | `App.svelte:251-288` | В capture нет бейджа комментариев и входа в окно задачи (CONCEPT:57-58) | Добавить бейдж + кнопку открытия |
| L4 | `Calendar.svelte:129-143` | Нет индикатора «у оригинала есть переносы» (CONCEPT:46-51) | `EXISTS (SELECT 1 FROM tasks w WHERE w.source_id = t.id)` |
| L5 | `Calendar.svelte:79-99` | Таймер тултипа не очищается при скрытии окна; после Esc+показ возможен «залипший» тултип | Сбрасывать в `visibilitychange`/на hide |
| L6 | `Calendar.svelte:376-387` | Фон чипов переноса берётся из `counts`, который грузится только в режиме «Год» → stale heat; уже перенесённые дни не помечены, клик молча no-op | Загружать counts для месяца или не красить чипы |
| L7 | `Calendar.svelte:161-182,209-212` | `transferring` не сбрасывается при смене месяца/года и `openDay` | Сбрасывать в `step`/`openDay`/смене режима |
| L8 | `App.svelte:44-73,53-64`, `Calendar.svelte:232-244` | `db.execute()` возвращает `QueryResult {rowsAffected, lastInsertId}`, а код сравнивает с `0` → проверки `rowsAffected === 0` мёртвы (и ветка «дубль — молча игнорируем») | `(await db.execute(...)).rowsAffected`; либо удалить мёртвую ветку |
| L9 | `Calendar.svelte:155-159,258-268`; `214-222` | Двойная загрузка на старте (onMount + `$effect`); `toggleDone` → `loadMonth()` + `emit` → listener грузит ещё раз; при смене месяца `rows` не очищается (мигание старых данных) | Убрать дубли, очищать `rows`, добавить loading |
| L10 | `App.svelte:33-42` | Нет ticket-guard как в Calendar → поздний ответ `refresh()` может перезаписать новый список; при ошибке `tasks` не очищается | Тот же паттерн `loadTicket` |
| L11 | `App.svelte:272` vs `Calendar.svelte:336` | Нумерация `i+1` в capture vs `position` в календаре | Использовать `position` в обоих |
| L12 | `lib.rs:340-346` | Пункт трея «Новая задача» эмитит `capture-shown`, а шорткат — `new-task-shortcut` (сейчас без последствий) | Эмитить одинаковое событие |
| L13 | `lib.rs:325-336` | Иконка трея = 1024×1024 цветной app-icon без `.icon_as_template(true)` → цветной квадрат в menu bar вместо монохромной иконки | Отдельный tray-asset 22×22 + template |
| L14 | `lib.rs:294-303` | `open_task_window` без `.center()` (остальные окна центрированы) | Добавить `.center()` |
| L15 | `Settings.svelte:59-89` | Оба рекордера можно включить одновременно (клик по второй кнопке) — клавиши перехватывает ветка capture | Взаимоисключающее включение |
| L16 | `db.ts:6-13`, `AGENTS.md:36-37` | `getDb()` в каждом окне вызывает plugin `load`, который **создаёт новый pool** и перезаписывает запись (`tauri-plugin-sql-2.6.0/src/commands.rs:26-46`); PRAGMA применяется к одному соединению пула, а FK и так ON по умолчанию в sqlx. Комментарий «одно соединение» вводит в заблуждение | Убрать PRAGMA/уточнить комментарий; знать, что pool на окно пересоздаётся |
| L17 | `lib.rs:40-97` | Down-миграции вкомпилированы, но `migrations()` их игнорирует (`_down`), а `rollback.sh` читает файлы с диска → риск рассинхрона/лишние байты в бинаре | Убрать down из `MIGRATIONS` (или использовать); `type MigrationDef` объявить выше |
| L18 | `app.css:23-32` | Глобальный `user-select: none`; `.text` не переопределяет → текст задач в capture/календаре нельзя выделить и скопировать (в TaskView можно) | `user-select: text` для `.text` |
| L19 | `lib.rs:187-256` | `Shortcut::from_str("D")` валиден → вызов `set_shortcut` из JS зарегистрирует глобальный перехват одиночной клавиши (UI такое не даёт, но Rust не валидирует) | Проверять непустые модификаторы в Rust |
| L20 | `package.json:16`, `Cargo.toml:18-19`, `capabilities/default.json:6-17` | Не используются: `@tauri-apps/plugin-opener`, `serde`/`serde_json`; лишние права `core:window:allow-show/set-focus`, `global-shortcut:allow-register/unregister`, дубли `sql:allow-*` | Удалить лишнее (least privilege) |
| L21 | `lib.rs:114-134,260-280,285-306`; `App.svelte:214,236`, `Calendar.svelte:185` | Гонка двойного создания окна: второй параллельный `invoke` получит ошибку build, а JS-вызовы без catch → unhandled rejection | Ловить ошибку и фокусировать существующее окно |
| L22 | `Settings.svelte:31-57,91-95` | Комбинация применяется, затем persist; при сбое записи статус err, но шорткат уже активен (рассинхрон до перезапуска). Если БД не загрузилась, `persist` молча return, статус «Сохранено» | Persist до применения или откат; показывать ошибку |
| L23 | `TaskView.svelte:89-94,124-133` | Окно задачи не слушает изменения (правка текста в capture не видна); черновик комментария переживает hide, но автофокуса при показе нет | Слушать `tasks-changed`, фокус на show |
| L24 | `Calendar.svelte:124-153` | `substr(day,1,4) = $1` не использует `idx_tasks_day` → full scan; коррелированный COUNT комментариев на строку | Диапазон `day >= 'YYYY-01-01' AND day < 'YYYY+1-01-01'`; при росте — LEFT JOIN GROUP BY |
| L25 | `Calendar.svelte:141` | Дни месяца сортируются по убыванию (`day DESC`) — не описано в документации | Зафиксировать в CONCEPT или поменять на asc |
| L26 | `Calendar.svelte:130-141`, `CONCEPT.md:40` | substr(…,1,40) не схлопывает переводы строк + `white-space: pre-wrap` → «однострочный» предпросмотр бывает многострочным | `replace(replace(text, char(10), ' '), char(13), ' ')` |
| L27 | `App.svelte:116-126` | Esc при `adding`/`editing` молча стирает draft/правку, а hide по шорткату — нет; несогласованно | Единое поведение/подтверждение |
| L28 | `Settings.svelte:167-169` | Статус без `aria-live`; ошибки parse из global-hotkey приходят на английском | `aria-live="polite"`, локализовать текст |
| L29 | `lib.rs:21-33,396-408` | `impl Default` для `ActiveShortcut`/`ActiveNewTaskShortcut` не используются; дефолтные константы парсятся 6 раз с `unwrap()` (опечатка = panic при старте) | `.manage(ActiveShortcut::default())`, единый хелпер + тест |
| L30 | `lib.rs:166-256` | Дублирование логики двух сеттеров, bool-параметры `excluding_*` | Общий helper + enum |
| L31 | `lib.rs:99-110,341-355,437-454` | Ошибки глотаются (`let _ =`), только `eprintln!`; у .app из Finder stderr не виден | `tauri-plugin-log`/файл-лог + показ ошибок в UI |
| L32 | `tauri.conf.json:31-35` | `bundle.icon: []`: для релизного `.app`/dmg иконка не настроена (README признаёт); `targets: "all"` | `npm run tauri icon app-icon.png` перед релизом; README уже говорит |
| L33 | `main.rs:1-6`, `Cargo.toml` | Остался `windows_subsystem` из шаблона (проект macOS-only); нет MSRV; нет CHANGELOG/LICENSE/tags; нет `.editorconfig` | Подчистить |
| L34 | git | WIP не закоммичен: `lib.rs` ссылается на untracked `0007_*.sql`, `docs/` целиком untracked | Коммитить парами (миграция + регистрация), `git add docs/` |
| L35 | `README.md:9-12,33` | `npm install` вместо `npm ci`; «БД в `app_data_dir`» — plugin использует `app_config_dir` (на macOS тот же путь, но формулировка неточна) | Поправить README |
| L36 | `Calendar.svelte:303-313` | 365 кнопок в обычном tab-порядке | Roving tabindex / grid-роль |
| L37 | `lib.rs` | `cargo fmt --check` расходится; clippy 12 warnings | `cargo fmt && cargo clippy --fix` |

---

## Что работает хорошо

- **Архитектура:** один SPA, роутинг по label окна (`main.ts:9-18`) — просто и предсказуемо; окна переиспользуются по label; fallback-открытие Settings при отказе шортката.
- **Безопасность:** все SQL-запросы параметризованы, динамической интерполяции нет; `{@html}`/`eval`/`innerHTML` отсутствуют (XSS исключён); строгий CSP; приложение полностью локальное, без сети; `dangerousRemoteDomainIpcAccess` не включён.
- **Шорткаты в Rust:** сравнение по значению (`Shortcut: PartialEq`), корректный порядок `register(new) → unregister(current)` с откатом нового при ошибке снятия старого (`lib.rs:202-214,239-249`); проверка взаимных конфликтов; обработка poisoned-lock; UI-рекордер работает через `e.code` (layout-independent) и покрыт тестами.
- **Данные:** миграции up/down файлами с правилом «не менять применённые», checksums целы; `is_transfer` — generated column (не рассинхронизируется); partial unique index + `NOT EXISTS` guard против повторного переноса; комментарии и бейджи резолвятся в оригинал; `TaskView` разворачивает перенос на оригинал.
- **Календарь:** аккуратные stale-guard'ы (`loadTicket` + проверки mode/year/month), корректные `$effect`-зависимости, container queries для сетки года, склонение «задача/задачи/задач» — приятные детали.
- **Документация:** для такого размера написана хорошо — CONCEPT/CONTEXT/AGENTS/design_review, инварианты зафиксированы, решения объяснены; тесты на чистые хелперы; сборка и тесты зелёные.

---

## План исправлений

**P0 — до следующего коммита/релиза (полдня):**
1. H1: `"closable": false` + `CloseRequested → prevent_close + hide` (10 минут, предотвращает потерю доступа к приложению).
2. H2: слушать `tasks-changed` в App, `refresh + todayStr` на `capture-shown`, `emit` в `doTransfer`.
3. M1: JOIN предпросмотра переносов в capture.
4. M4: in-flight guard в `commitDraft`/`saveEdit`/`submitComment`, захват `editing`.
5. M5: `.catch` на `getDb()` + видимый error-state.
6. M2: вернуть каноническую строку (или нормализовать display во фронте).
7. Закоммитить WIP (0007 + docs).

**P1 — качество и инварианты (1–2 дня):**
- M3: решить модель done (оригинал vs per-link) и описать в CONTEXT/CONCEPT.
- M6: перенос между месяцами (навигация в пикере).
- M7/M8: честный state шорткатов; `connect-src ipc:`.
- M9: CHECK/UNIQUE(day,position)/запрет DELETE + retry на `SQLITE_BUSY`.
- M10: `sqlite3 .backup`, pre-flight, транзакции, автобэкап при старте, README-раздел «данные/бэкап».
- M11: синхронизировать CONCEPT/CONTEXT/README/AGENTS (`4×3`→`3×4`, is_done, settings, команды, `$N`).
- M12: убрать `role=button` с контейнеров или guard по `e.target`; `:focus-visible`; aria для ячеек года.
- M13: `svelte-check` + `vite-env.d.ts` + скрипты `check`/`lint` + CI (`npm test`, `npm run build`, `cargo fmt --check`, `cargo clippy`, `cargo test`); тесты на SQL-инварианты/чистые хелперы (`taskCountWord`, `buildMonthCells`, `shouldRollover`) и Rust round-trip шортката.

**P2 — low/nit:** L1–L37 по мере касания кода (особенно L1 — закрытие task-окон, L2 single-instance, L18 user-select, L21 гонка создания окон).

**Быстрые победы (мелко, но заметно):** `cargo fmt` + `cargo clippy --fix`; удалить неиспользуемые зависимости/права; `connect-src`; `.backup` в rollback; `:focus-visible`; `user-select: text` для текста задач; коммит WIP.

---

## Приложение: аудит живой БД (копия)

- 7 миграций, все `success=1`; `_sqlx_migrations.checksum` = sha384 соответствующих up-файлов (правило «не менять применённые» соблюдено).
- Схема: `tasks(id, day, position, text, source_id, is_transfer VIRTUAL, created_at, updated_at, is_done)` + индексы `idx_tasks_day`, `idx_tasks_source`, `idx_tasks_transfer (UNIQUE partial)`, `comments`, `settings`.
- `journal_mode = wal`; FK ON; `_sqlx_migrations` в порядке версий.
- Данные: 5 tasks (2 done), 1 comment, 2 settings (`capture_shortcut=Alt+Shift+N`, `new_task_shortcut=Alt+Shift+T`) — именно эти значения сейчас отображаются в настройках как «shift alt KeyN»/«shift alt KeyT» (см. M2).
- Integrity: 0 дублей переносов, 0 переносов-из-переносов, 0 self-source, 0 комментариев к переносам, 0 битых `source_id`, 0 дублей `position`, 0 задач с неверным форматом `day`.
