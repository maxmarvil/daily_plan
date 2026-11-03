use tauri::{Manager, WebviewWindow};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, Code, Modifiers, ShortcutState};
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

/// Глобальный шорткат показа/скрытия quick-capture окна.
const CAPTURE_SHORTCUT: &str = "CmdOrCtrl+Shift+D";

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shortcuts =
        ShortcutBuilder::new().with_shortcuts([CAPTURE_SHORTCUT]).expect("invalid shortcut");

    tauri::Builder::default()
        .plugin(
            SqlBuilder::default()
                .add_migrations("sqlite:daily-plan.db", migrations())
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(
            shortcuts
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    if shortcut.matches(Modifiers::SUPER | Modifiers::SHIFT, Code::KeyD)
                        || shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyD)
                    {
                        if let Some(win) = app.get_webview_window("main") {
                            toggle_capture_window(&win);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            let _ = app;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_calendar])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
