use std::sync::Mutex;
use tauri::{AppHandle, Manager, State, WebviewWindow};
use tauri_plugin_global_shortcut::{
    Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState,
};
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

/// Комбинация по умолчанию; реальная живёт в managed-state и таблице settings.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+D";

/// Сейчас зарегистрированная глобальная комбинация вызова окна.
#[derive(Default)]
struct ActiveShortcut(Mutex<String>);

const MIGRATION_TASKS: &str = r#"
CREATE TABLE IF NOT EXISTS tasks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  day         TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  text        TEXT    NOT NULL DEFAULT '',
  source_id   INTEGER REFERENCES tasks(id),
  is_transfer INTEGER GENERATED ALWAYS AS (source_id IS NOT NULL) VIRTUAL,
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL
)"#;

const MIGRATION_TASKS_INDEXES: &str = r#"
CREATE INDEX IF NOT EXISTS idx_tasks_day ON tasks(day, position);
CREATE INDEX IF NOT EXISTS idx_tasks_source ON tasks(source_id)"#;

const MIGRATION_COMMENTS: &str = r#"
CREATE TABLE IF NOT EXISTS comments (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  task_id    INTEGER NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
  text       TEXT    NOT NULL,
  created_at TEXT    NOT NULL
)"#;

const MIGRATION_COMMENTS_INDEX: &str =
  "CREATE INDEX IF NOT EXISTS idx_comments_task ON comments(task_id)";

// одна и та же задача не может быть перенесена в один день дважды
const MIGRATION_TRANSFER_UNIQUE: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_transfer
ON tasks(source_id, day) WHERE source_id IS NOT NULL"#;

const MIGRATION_SETTINGS: &str = r#"
CREATE TABLE IF NOT EXISTS settings (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
)"#;

fn migrations() -> Vec<Migration> {
    vec![
        Migration {
            version: 1,
            description: "create_tasks",
            sql: MIGRATION_TASKS,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "tasks_indexes",
            sql: MIGRATION_TASKS_INDEXES,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 3,
            description: "create_comments",
            sql: MIGRATION_COMMENTS,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 4,
            description: "comments_index",
            sql: MIGRATION_COMMENTS_INDEX,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 5,
            description: "transfer_unique",
            sql: MIGRATION_TRANSFER_UNIQUE,
            kind: MigrationKind::Up,
        },
        Migration {
            version: 6,
            description: "create_settings",
            sql: MIGRATION_SETTINGS,
            kind: MigrationKind::Up,
        },
    ]
}

fn toggle_capture_window(win: &WebviewWindow) {
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);

    if visible && focused {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// Окно настроек.
#[tauri::command]
fn open_settings(app: tauri::AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("settings") {
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }

    let win = tauri::WebviewWindowBuilder::new(
        &app,
        "settings",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Настройки")
    .inner_size(520.0, 600.0)
    .min_inner_size(420.0, 420.0)
    .center()
    .build()?;
    win.set_focus()?;
    Ok(())
}

fn current_shortcut(app: &AppHandle) -> String {
    app.state::<ActiveShortcut>().0.lock().unwrap().clone()
}

#[tauri::command]
fn get_shortcut(state: State<'_, ActiveShortcut>) -> String {
    state.0.lock().unwrap().clone()
}

/// Меняет глобальную комбинацию: сначала регистрируем новую, и только
/// после успеха снимаем старую — при ошибке остаёмся на рабочей клавише.
#[tauri::command]
fn set_shortcut(app: AppHandle, shortcut: String) -> Result<String, String> {
    let new = shortcut.trim().to_string();
    if new.is_empty() {
        return Err("Комбинация пуста".into());
    }

    let current = current_shortcut(&app);
    if current == new {
        return Ok(new);
    }

    app.global_shortcut()
        .register(new.as_str())
        .map_err(|e| format!("Не удалось зарегистрировать «{new}»: {e}"))?;

    let _ = app.global_shortcut().unregister(current.as_str());
    *app.state::<ActiveShortcut>().0.lock().unwrap() = new.clone();
    Ok(new)
}

/// Показывает окно календаря, создавая его при первом вызове.
#[tauri::command]
fn open_calendar(app: tauri::AppHandle) -> tauri::Result<()> {
    if let Some(win) = app.get_webview_window("calendar") {
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }

    let win = tauri::WebviewWindowBuilder::new(
        &app,
        "calendar",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Календарь")
    .inner_size(720.0, 760.0)
    .min_inner_size(560.0, 600.0)
    .center()
    .build()?;
    win.set_focus()?;
    Ok(())
}

/// Окно просмотра задачи: текст + лента комментариев.
/// Одно окно на задачу (label `task-{id}`), повторный вызов фокусирует его.
#[tauri::command]
fn open_task_window(app: tauri::AppHandle, task_id: i64) -> tauri::Result<()> {
    let label = format!("task-{task_id}");

    if let Some(win) = app.get_webview_window(&label) {
        win.show()?;
        win.set_focus()?;
        return Ok(());
    }

    let win = tauri::WebviewWindowBuilder::new(
        &app,
        &label,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("Задача")
    .inner_size(420.0, 560.0)
    .min_inner_size(320.0, 360.0)
    .build()?;
    win.set_focus()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(
            SqlBuilder::default()
                .add_migrations("sqlite:daily-plan.db", migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    // плагин используется только для вызова capture-окна,
                    // поэтому реагируем на любое нажатие зарегистрированной combo
                    if event.state == ShortcutState::Pressed {
                        if let Some(win) = app.get_webview_window("main") {
                            toggle_capture_window(&win);
                        }
                    }
                })
                .build(),
        )
        .manage(ActiveShortcut(Mutex::new(
            DEFAULT_SHORTCUT.to_string(),
        )))
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Err(e) = app
                .handle()
                .global_shortcut()
                .register(DEFAULT_SHORTCUT)
            {
                eprintln!("не удалось зарегистрировать шорткат по умолчанию: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_calendar,
            open_task_window,
            open_settings,
            get_shortcut,
            set_shortcut
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
