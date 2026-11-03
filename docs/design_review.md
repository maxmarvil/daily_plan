# Design review notes

This file collects concrete design decisions and UI fixes applied to Daily Plan.

## Capture window (`src/App.svelte`)

### Add-task button

- **Height**: 45 px — уменьшена на четверть от прежних 60 px.
- **Shape**: capsule / balloon (`border-radius: 9999px`).
- **Width**: 4 × height = 180 px.
- **Icon**: `+`, font-size 24 px, centered both horizontally and vertically.
- **Alignment**: centered horizontally within `.bar` via `display: flex; justify-content: center`.
- Implementation uses a CSS custom property `--add-h` so width is always `calc(var(--add-h) * 4)`.

### New-task global shortcut

- Separate configurable global shortcut for instant new-task input.
- Default: `CmdOrCtrl+Shift+N`; константы синхронизированы между `src-tauri/src/lib.rs` и `src/lib/shortcut.ts`.
- Rust state stores parsed `Shortcut` objects (not raw strings) so the handler can compare by value and avoid platform-format mismatches (`CmdOrCtrl` vs `Command`).
- Commands: `get_new_task_shortcut` / `set_new_task_shortcut`.
- Handler distinguishes capture shortcut from new-task shortcut and emits `new-task-shortcut` event.
- Capture window listens for the event and sets `adding = true`.
- Tray item «Новая задача» emits the same event, so it also activates new-task input.
- Settings page (`src/Settings.svelte`) has a second row to configure it.
- Conflicts with the capture shortcut are rejected.
- Shortcut combinations are displayed as separate key-cap chips (`formatComboParts` + `.key` spans).

### Header icons (settings, calendar)

- Rendered size: **30 × 30 px** (twice the previous 15 × 15 px).
- Keep the icon SVG `viewBox="0 0 20 20"`; only `width`/`height` attributes were doubled.
- **Settings icon**: replaced with a classic gear (cog) shape; previous icon resembled a ladybug.

## Calendar window (`src/Calendar.svelte`)

### Year view grid

- Layout: **3 columns × 4 rows** (was 4 × 3).
- CSS: `.year-grid { grid-template-columns: repeat(3, minmax(0, 1fr)); }`.
- Размеры шрифтов — `clamp()` c `vw` (`font-size: clamp(12px, 2.2vw, 20px)` у номера дня и т.п.). **Container queries убраны** (`container-type`/`cqi`): WebKit теряет grid `gap` после увеличения + повторного сужения окна, если в поддереве есть containment. Зазоры сделаны структурно — `padding` у месяца и `margin: 1px` у ячеек дня.
- **Day numbers enlarged**: `.cell { font-size: clamp(12px, 2.2vw, 20px) }` (было 8–13 px).
  У кнопки-ячейки сброшен нативный `padding` — иначе её `min-content` шире трека и
  последняя колонка наезжает на соседний месяц.
- **Gaps**: между номерами дней 2 px (margin 1 px у ячейки, не grid gap); между месяцами 29 px по вертикали и 31 px по горизонтали (padding 14.5/15.5 px), чтобы блоки месяцев визуально не сливались; outer padding 8/12/14 px.
- **Minimum year width**: `minmax(219px, 1fr)` per month (188 px content + 31 px padding) and
  `min-width: 657px` (219 × 3); `.year-scroll` uses `overflow: auto`, so when the window gets
  narrower a horizontal scrollbar appears instead of month tiles overlapping.
  Calendar window minimum width lowered 560 → 360 px so this is reachable.
- Calendar header wraps (`flex-wrap`) so navigation/segmented control do not collide in a narrow window.

### Day tooltip in year view

- Tooltip appears only on days with tasks (`counts[day] > 0`).
- Content: `<дата>: <N> <слово "задача" в правильном склонении>`.
- Custom tooltip element (`.year-tooltip`) positioned above the hovered cell.
- **Delay**: 1 second (`setTimeout(..., 1000)`) before showing; hides immediately on `mouseleave`.
- Implemented with `mouseenter`/`mouseleave` handlers instead of the native `title` attribute for predictable timing.

### Task done state

- New column `tasks.is_done INTEGER NOT NULL DEFAULT 0` (migration `0007_add_task_is_done`).
- `is_done` belongs to the **original** task: a transfer row displays and toggles
  the original's state (`id = COALESCE(source_id, id)`); one task cannot be done
  in one day and not done in another.
- Done is allowed for tasks of **any** day, including past days; editing the text
  of past tasks is still forbidden (UI rule).
- Each task row has a **done / return** button:
  - Not done: green checkmark (`✓`) labeled «Отметить выполненной».
  - Done: return arrow (`↩`) labeled «Вернуть в работу».
- Visual states:
  - Default: thin **orange** inset outline (`var(--accent)`).
  - Done: thin **green** inset outline (`var(--success)`), gray struck-through text.
- Button appears on hover (or stays visible when done) to avoid clutter.
- Same behavior in `src/App.svelte` capture list and `src/Calendar.svelte` month list.
- Year heatmap counts done tasks as activity.
- Task rows have `margin-bottom: 5px` for spacing between tasks.

## 2026-11-03 — исправления code review

Сводка правок после ревью (полный разбор находок — `docs/code_review.md`).

### Accessibility (M12)

- Строки задач в capture и «Месяце» — настоящие `<button>`; убрана вложенность
  `div role="button"` с кнопками внутри, поэтому Enter на кнопке done больше не
  открывает редактор/оригинал заодно.
- Добавлен `:focus-visible` для кнопок-действий: с клавиатуры видно, что в фокусе,
  хотя hover-скрытие (`opacity: 0`) осталось.
- Ячейки года получили `aria-label` (дата + число задач); сегменты режима/кнопки-
  переключатели — `aria-pressed`; статусы в настройках — `aria-live="polite"`.

### Done у оригинала (M3)

- `is_done` — свойство оригинала: строка-перенос отображает и переключает
  состояние оригинала (`WHERE id = COALESCE(source_id, id)`).
- Done доступен задачам любого дня, включая прошлые; текст прошлых задач
  по-прежнему не редактируется.
- «Год» считает выполненные задачи активностью.

### Capture: переносы и бейджи (M1, L3)

- Переносы рендерятся предпросмотром оригинала (первые 40 символов, переводы
  строк схлопнуты) через `LEFT JOIN tasks s ON s.id = t.source_id` — вместо
  пустых оранжевых плашек.
- У задач в capture появились бейдж комментариев `💬` и кнопка открытия окна
  задачи; пустой день показывает пустое состояние.

### Индикатор оригиналов с переносами (L4)

- Запрос месяца считает переносы подзапросом
  `(SELECT COUNT(*) FROM tasks w WHERE w.source_id = t.id) AS transfers`.
- У оригинала с хотя бы одним переносом — ненасыщенный оранжевый индикатор
  (точка/полоска), без фона: задача своего дня, а гость — ссылка.

### Навигация пикера переноса (M6, L6)

- Пикер в «Месяце» листает месяцы (`‹`/`›`) — перенос возможен в другой месяц.
- Уже перенесённые дни помечены и недоступны для повторного переноса.
- После успешного переноса календарь переходит на месяц цели.

### Запрет закрытия main-окна (H1)

- `main` получил `"closable": false`; `CloseRequested` перехватывается
  (`prevent_close` + `hide`).
- Capture всегда возвращается шорткатом или пунктом трея; окна задач
  закрываются по `Esc` (не скрываются) и переоткрываются заново, скрытые
  webview не копятся.

### Ошибки БД (M5)

- При сбое загрузки/записи окна показывают видимый баннер с текстом ошибки;
  список не выдаёт ошибку за «задач нет».

## How to update this file

When a design change is requested and applied, add a short entry here with the affected component, the decision, and the file/selector changed. Keep the existing sections up to date if the same element is modified again.
