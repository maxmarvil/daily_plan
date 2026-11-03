use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_global_shortcut::{
    Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState,
};
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

/// Комбинация по умолчанию; реальная живёт в managed-state и таблице settings.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+D";

/// Сейчас зарегистрированная глобальная комбинация вызова окна.
#[derive(Default)]
struct ActiveShortcut(Mutex<String>);

/// Миграции живут в `migrations/NNNN_name.up.sql` / `.down.sql`.
/// tauri-plugin-sql выполняет только Up; down-скрипты использует
/// `scripts/rollback.sh` для ручного отката (см. README).
///
/// Новая миграция: создать пару файлов, добавить строку в этот список.
const MIGRATIONS: &[MigrationDef] = &[
    (
        1,
        "create_tasks",
        include_str!("../migrations/0001_create_tasks.up.sql"),
        include_str!("../migrations/0001_create_tasks.down.sql"),
    ),
    (
        2,
        "tasks_indexes",
        include_str!("../migrations/0002_tasks_indexes.up.sql"),
        include_str!("../migrations/0002_tasks_indexes.down.sql"),
    ),
    (
        3,
        "create_comments",
        include_str!("../migrations/0003_create_comments.up.sql"),
        include_str!("../migrations/0003_create_comments.down.sql"),
    ),
    (
        4,
        "comments_index",
        include_str!("../migrations/0004_comments_index.up.sql"),
        include_str!("../migrations/0004_comments_index.down.sql"),
    ),
    (
        5,
        "transfer_unique",
        include_str!("../migrations/0005_transfer_unique.up.sql"),
        include_str!("../migrations/0005_transfer_unique.down.sql"),
    ),
    (
        6,
        "create_settings",
        include_str!("../migrations/0006_create_settings.up.sql"),
        include_str!("../migrations/0006_create_settings.down.sql"),
    ),
];

type MigrationDef = (i64, &'static str, &'static str, &'static str);

fn migrations() -> Vec<Migration> {
    MIGRATIONS
        .iter()
        .map(|(version, description, up, _down)| Migration {
            version: *version,
            description: (*description).into(),
            sql: (*up).into(),
            kind: MigrationKind::Up,
        })
        .collect()
}

fn toggle_capture_window(app: &AppHandle, win: &WebviewWindow) {
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);

    if visible && focused {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
        let _ = app.emit("capture-shown", ());
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
    .always_on_top(true)
    .center()
    .build()?;
    win.set_focus()?;
    Ok(())
}

fn current_shortcut(app: &AppHandle) -> Result<String, String> {
    app.state::<ActiveShortcut>()
        .0
        .lock()
        .map(|s| s.clone())
        .map_err(|_| "shortcut state poisoned".to_string())
}

#[tauri::command]
fn get_shortcut(state: State<'_, ActiveShortcut>) -> Result<String, String> {
    state.0.lock().map(|s| s.clone()).map_err(|_| {
        "shortcut state poisoned".to_string()
    })
}

/// Меняет глобальную комбинацию: сначала регистрируем новую, и только
/// после успеха снимаем старую — при ошибке остаёмся на рабочей клавише.
#[tauri::command]
fn set_shortcut(app: AppHandle, shortcut: String) -> Result<String, String> {
    let new = shortcut.trim().to_string();
    if new.is_empty() {
        return Err("Комбинация пуста".into());
    }

    let current = current_shortcut(&app)?;
    if current == new {
        return Ok(new);
    }

    app.global_shortcut()
        .register(new.as_str())
        .map_err(|e| format!("Не удалось зарегистрировать «{new}»: {e}"))?;

    // Если снять старый не удалось — откатываем новый, чтобы не остаться
    // с двумя рабочими комбинациями и рассинхроном state.
    if let Err(e) = app.global_shortcut().unregister(current.as_str()) {
        let _ = app.global_shortcut().unregister(new.as_str());
        return Err(format!(
            "Новая комбинация зарегистрирована, но не удалось снять старую «{current}»: {e}"
        ));
    }

    *app.state::<ActiveShortcut>()
        .0
        .lock()
        .map_err(|_| "shortcut state poisoned")? = new.clone();
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
    .always_on_top(true)
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
    .always_on_top(true)
    .build()?;
    win.set_focus()?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    if let Err(e) = tauri::Builder::default()
        .plugin(
            SqlBuilder::default()
                .add_migrations("sqlite:daily-plan.db", migrations())
                .build(),
        )
        .plugin(
            ShortcutBuilder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        if let Some(win) = app.get_webview_window("main") {
                            toggle_capture_window(app, &win);
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
                // fallback: показываем настройки, чтобы пользователь мог
                // назначить другую комбинацию (главное окно стартует скрытым)
                let _ = open_settings(app.handle().clone());
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
    {
        eprintln!("error while running tauri application: {e}");
        std::process::exit(1);
    }
}
