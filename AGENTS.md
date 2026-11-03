# Daily Plan — agent guide

## Stack

- **Tauri v2** + **Rust** (`src-tauri/`)
- **Svelte 5** + **Vite** + **TypeScript** (`src/`)
- **SQLite** via `tauri-plugin-sql`
- Target platform: **macOS**

## Quick commands

```bash
# Install deps exactly per lockfile
npm ci

# Dev (Vite + Tauri, hot reload)
npm run tauri dev

# Typecheck (svelte-check: TypeScript + Svelte)
npm run check

# Build frontend only
npm run build

# Release bundles (.app + .dmg); details — docs/release.md
npm run release
scripts/build-macos.sh

# Run tests
npm test

# Rust checks (same as CI)
(cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test)
```

CI: `.github/workflows/ci.yml` runs `npm test`, `npm run check`, `npm run build`,
plus `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`.

## Architecture

- **Single-page app routed by window label** (`src/main.ts`):
  - `main` → `App.svelte` (capture)
  - `calendar` → `Calendar.svelte`
  - `settings` → `Settings.svelte`
  - `task-{id}` → `TaskView.svelte`
- Rust commands live in `src-tauri/src/lib.rs`:
  - windows: `open_calendar`, `open_settings`, `open_task_window` → `tauri::Result<()>`
  - shortcuts: `get_shortcut`, `set_shortcut`, `get_new_task_shortcut`,
    `set_new_task_shortcut` → `Result<String, String>` (`Ok` = canonical combo,
    `Err` = user-facing message)
  - backup: `prepare_backup(day)` → `Result<Option<String>, String>` (target
    path for today's backup; `None` if it already exists; the `VACUUM INTO`
    itself runs from the frontend), `prune_backups(keep)` → `Result<(), String>`
    (keeps the newest 7)
- DB access goes through the `src/lib/db.ts` singleton per window. The plugin's
  `load()` creates a **sqlx pool** per window; FK enforcement is ON by default in
  sqlx, and the `PRAGMA foreign_keys = ON` in `db.ts` only lands on one
  connection of that pool (harmless, but not the guarantee).
- Cross-window refresh is done via `emit("tasks-changed")` and `listen`.
- Shared task queries/helpers live in `src/lib/tasks.ts`.

## Migrations

- Files: `src-tauri/migrations/NNNN_name.up.sql` and `.down.sql`
- Register new migrations in `src-tauri/src/lib.rs` → `MIGRATIONS`.
- Up migrations run automatically via `tauri-plugin-sql` (only Up; plugin
  does not run Down).
- Manual rollback: `scripts/rollback.sh <target_version>` — checks the app is
  closed, preflights all down files, backs up with `sqlite3 .backup`, applies each
  down migration in its own transaction (a mid-way failure leaves a partial
  rollback; restore from the backup). Requires sqlite3 ≥ 3.35.
- **Never modify content of already-applied migrations** — sqlx checksums
  will break. Add new migrations instead.
- Schema invariants are enforced in `0008`: `UNIQUE(day, position)` and triggers
  for `day` format, text/source rules, immutability of `day`/`created_at`/
  `source_id`, `is_done ∈ {0,1}`, comments only to originals, and `DELETE` ban.
  The «own day» edit rule stays UI-only — all task CRUD comes from JS.
- `is_done` was added in `0007`; it belongs to the original task, transfers
  reflect/toggle it.

## Conventions

### TypeScript / Svelte

- Use **Svelte 5 runes**: `$state`, `$derived`, `$effect`.
- DB placeholders are **positional**. With sqlx-sqlite, `$1` may be repeated
  and counts as **one** parameter (total params = max index), e.g. `$4` twice
  still needs four values. The "each occurrence is a separate parameter" rule
  applies to `?`. Prefer distinct indices for readability.
- TypeScript is pinned to **6.x**: TS 7 (native) has no JS API that
  `svelte-check` needs.
- Async effects in `$effect` must guard against stale results when state
  changes quickly (see `Calendar.svelte` `loadTicket` pattern).

### Rust

- Window commands return `tauri::Result<()>`; shortcut and backup commands return
  `Result<T, String>` (shortcut `Ok` = canonical combo string, `Err` =
  user-facing message).
- Avoid `unwrap()`/`expect()` in command bodies; handle `Mutex` poisoning.
- Window helpers reuse existing windows (`get_webview_window`) and set focus.
- Shortcut state stores parsed `Shortcut` values (compare by value), not raw
  strings — avoids platform-format mismatches.

### Styling

- macOS Notes-like look: system font, rounded corners, subtle hover states,
  light/dark via `prefers-color-scheme`.
- Accent color is orange (`--accent`). Transfer rows use it for background.

## Testing

- Unit tests for pure helpers: `src/lib/*.spec.ts`.
- Run with `npm test` (vitest + happy-dom).
- Typecheck with `npm run check` (svelte-check; TypeScript 6.x).
- There are no UI/integration tests yet.

## Common pitfalls

- `core:default` does **not** include all window actions; add explicit
  permissions in `capabilities/default.json` when using `hide`/`show`/`set_focus`.
- `main` cannot be closed (`closable: false` + `CloseRequested` → hide); don't
  rely on destroy/recreate of the capture window. Task windows close on Esc.
- FK enforcement is per sqlx connection and ON by default; use `getDb()` so each
  window reuses its configured client instead of calling `Database.load` ad hoc.
- Global shortcuts are layout-sensitive: the recorder uses `e.code`, not
  `e.key`.
- The capture window starts hidden; if a default shortcut fails to register,
  the app opens Settings as a fallback.
- Tray menu requires `tray-icon` feature in `Cargo.toml` and a window icon.
- DB errors are surfaced as a visible banner in every window — keep the error
  state instead of silently rendering an empty list.

## Where to look

- Domain glossary / invariants: `CONTEXT.md`
- Product concept: `CONCEPT.md`
- DB schema: migrations + `CONTEXT.md` invariants
- DB invariants (constraints/triggers): `src-tauri/migrations/0008_*.up.sql`
- Window/shortcut logic: `src-tauri/src/lib.rs`
- Shared task queries/helpers: `src/lib/tasks.ts`
- UI components: `src/App.svelte`, `src/Calendar.svelte`,
  `src/TaskView.svelte`, `src/Settings.svelte`
- Design notes and applied UI fixes: `docs/design_review.md`
- Release/build/signing: `docs/release.md`, `scripts/build-macos.sh`
- CI: `.github/workflows/ci.yml`
