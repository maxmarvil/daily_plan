# Daily Plan — agent guide

## Stack

- **Tauri v2** + **Rust** (`src-tauri/`)
- **Svelte 5** + **Vite** + **TypeScript** (`src/`)
- **SQLite** via `tauri-plugin-sql`
- Target platform: **macOS**

## Quick commands

```bash
# Dev (Vite + Tauri, hot reload)
npm run tauri dev

# Build frontend only
npm run build

# Run tests
npm test

# Cargo check
(cd src-tauri && cargo check)
```

## Architecture

- **Single-page app routed by window label** (`src/main.ts`):
  - `main` → `App.svelte` (capture)
  - `calendar` → `Calendar.svelte`
  - `settings` → `Settings.svelte`
  - `task-{id}` → `TaskView.svelte`
- Rust commands live in `src-tauri/src/lib.rs`:
  - `open_calendar`, `open_settings`, `open_task_window`
  - `get_shortcut`, `set_shortcut`
- DB access goes through `src/lib/db.ts` singleton, which enables
  `PRAGMA foreign_keys = ON` on the one connection it keeps.
- Cross-window refresh is done via `emit("tasks-changed")` and `listen`.

## Migrations

- Files: `src-tauri/migrations/NNNN_name.up.sql` and `.down.sql`
- Register new migrations in `src-tauri/src/lib.rs` → `MIGRATIONS`.
- Up migrations run automatically via `tauri-plugin-sql` (only Up; plugin
  does not run Down).
- Manual rollback: `scripts/rollback.sh <target_version>` (backs up DB first).
- **Never modify content of already-applied migrations** — sqlx checksums
  will break. Add new migrations instead.

## Conventions

### TypeScript / Svelte

- Use **Svelte 5 runes**: `$state`, `$derived`, `$effect`.
- DB placeholders are **positional** (`$1`, `$2`, …). Each occurrence
  counts as a separate parameter, so `$4` used twice needs **five** values
  total. Prefer distinct indices.
- Async effects in `$effect` must guard against stale results when state
  changes quickly (see `Calendar.svelte` `loadTicket` pattern).

### Rust

- Commands return `Result<String, String>` for errors the UI should show.
- Avoid `unwrap()`/`expect()` in command bodies; handle `Mutex` poisoning.
- Window helpers reuse existing windows (`get_webview_window`) and set focus.

### Styling

- macOS Notes-like look: system font, rounded corners, subtle hover states,
  light/dark via `prefers-color-scheme`.
- Accent color is orange (`--accent`). Transfer rows use it for background.

## Testing

- Unit tests for pure helpers: `src/lib/*.spec.ts`.
- Run with `npm test` (vitest + happy-dom).
- There are no UI/integration tests yet.

## Common pitfalls

- `core:default` does **not** include all window actions; add explicit
  permissions in `capabilities/default.json` when using `hide`/`show`/`set_focus`.
- SQLite FK enforcement is per-connection; always use `getDb()` instead of
  `Database.load` directly.
- Global shortcuts are layout-sensitive: the recorder uses `e.code`, not
  `e.key`.
- The capture window starts hidden; if the default shortcut fails to register,
  the app opens Settings as a fallback.
- Tray menu requires `tray-icon` feature in `Cargo.toml` and a window icon.

## Where to look

- Domain glossary / invariants: `CONTEXT.md`
- Product concept: `CONCEPT.md`
- DB schema: migrations + `CONTEXT.md` invariants
- Window/shortcut logic: `src-tauri/src/lib.rs`
- UI components: `src/App.svelte`, `src/Calendar.svelte`,
  `src/TaskView.svelte`, `src/Settings.svelte`
