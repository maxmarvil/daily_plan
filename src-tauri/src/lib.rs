use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tauri_plugin_global_shortcut::{
    Builder as ShortcutBuilder, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState,
};
use tauri_plugin_sql::{Builder as SqlBuilder, Migration, MigrationKind};

/// Комбинация по умолчанию для вызова capture-окна.
pub const DEFAULT_SHORTCUT: &str = "CmdOrCtrl+Shift+D";
/// Комбинация по умолчанию для быстрого создания новой задачи.
pub const DEFAULT_NEW_TASK_SHORTCUT: &str = "CmdOrCtrl+Shift+N";

/// Каталог бэкапов внутри app_config_dir.
const BACKUP_DIR: &str = "backups";
/// Префикс файлов бэкапа: `daily-plan-YYYY-MM-DD.db`.
const BACKUP_PREFIX: &str = "daily-plan-";
/// Сколько последних бэкапов хранить.
const BACKUP_KEEP: u32 = 7;
/// Файл лога внутри app_config_dir.
const LOG_FILE: &str = "daily-plan.log";
/// Максимальный размер лога перед ротацией (перезаписью).
const MAX_LOG_BYTES: u64 = 1_000_000;

fn default_shortcut() -> Shortcut {
    Shortcut::from_str(DEFAULT_SHORTCUT)
        .expect("DEFAULT_SHORTCUT должен быть корректной комбинацией")
}

fn default_new_task_shortcut() -> Shortcut {
    Shortcut::from_str(DEFAULT_NEW_TASK_SHORTCUT)
        .expect("DEFAULT_NEW_TASK_SHORTCUT должен быть корректной комбинацией")
}

/// Сейчас зарегистрированная глобальная комбинация вызова окна.
///
/// `registered` важен: без него state «врёт» после неудачной регистрации
/// дефолта и сеттеры отвечают Ok, не зарегистрировав комбинацию.
struct ActiveShortcut {
    shortcut: Mutex<Shortcut>,
    registered: AtomicBool,
}

impl Default for ActiveShortcut {
    fn default() -> Self {
        Self {
            shortcut: Mutex::new(default_shortcut()),
            registered: AtomicBool::new(false),
        }
    }
}

impl ActiveShortcut {
    fn current(&self) -> Result<Shortcut, String> {
        self.shortcut
            .lock()
            .map(|s| *s)
            .map_err(|_| "shortcut state poisoned".to_string())
    }

    fn is_registered(&self) -> bool {
        self.registered.load(Ordering::SeqCst)
    }

    fn mark_registered(&self, shortcut: Shortcut) -> Result<(), String> {
        *self
            .shortcut
            .lock()
            .map_err(|_| "shortcut state poisoned")? = shortcut;
        self.registered.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Сейчас зарегистрированная глобальная комбинация создания новой задачи.
struct ActiveNewTaskShortcut {
    shortcut: Mutex<Shortcut>,
    registered: AtomicBool,
}

impl Default for ActiveNewTaskShortcut {
    fn default() -> Self {
        Self {
            shortcut: Mutex::new(default_new_task_shortcut()),
            registered: AtomicBool::new(false),
        }
    }
}

impl ActiveNewTaskShortcut {
    fn current(&self) -> Result<Shortcut, String> {
        self.shortcut
            .lock()
            .map(|s| *s)
            .map_err(|_| "new-task shortcut state poisoned".to_string())
    }

    fn is_registered(&self) -> bool {
        self.registered.load(Ordering::SeqCst)
    }

    fn mark_registered(&self, shortcut: Shortcut) -> Result<(), String> {
        *self
            .shortcut
            .lock()
            .map_err(|_| "new-task shortcut state poisoned")? = shortcut;
        self.registered.store(true, Ordering::SeqCst);
        Ok(())
    }
}

/// Миграции живут в `migrations/NNNN_name.up.sql` / `.down.sql`.
/// tauri-plugin-sql выполняет только Up; down-скрипты использует
/// `scripts/rollback.sh` для ручного отката (см. README).
///
/// Новая миграция: создать пару файлов, добавить up-строку в этот список
/// и версию в `scripts/rollback.sh`-совместимом виде (down читается с диска).
type MigrationDef = (i64, &'static str, &'static str);

const MIGRATIONS: &[MigrationDef] = &[
    (
        1,
        "create_tasks",
        include_str!("../migrations/0001_create_tasks.up.sql"),
    ),
    (
        2,
        "tasks_indexes",
        include_str!("../migrations/0002_tasks_indexes.up.sql"),
    ),
    (
        3,
        "create_comments",
        include_str!("../migrations/0003_create_comments.up.sql"),
    ),
    (
        4,
        "comments_index",
        include_str!("../migrations/0004_comments_index.up.sql"),
    ),
    (
        5,
        "transfer_unique",
        include_str!("../migrations/0005_transfer_unique.up.sql"),
    ),
    (
        6,
        "create_settings",
        include_str!("../migrations/0006_create_settings.up.sql"),
    ),
    (
        7,
        "add_task_is_done",
        include_str!("../migrations/0007_add_task_is_done.up.sql"),
    ),
    (
        8,
        "data_integrity",
        include_str!("../migrations/0008_data_integrity.up.sql"),
    ),
];

fn migrations() -> Vec<Migration> {
    MIGRATIONS
        .iter()
        .map(|(version, description, sql)| Migration {
            version: *version,
            description,
            sql,
            kind: MigrationKind::Up,
        })
        .collect()
}

/// Пишет сообщение в stderr и в `<app_config_dir>/daily-plan.log`.
/// Нужно потому, что у собранного .app stderr из Finder не виден.
fn log_line(app: &AppHandle, message: impl AsRef<str>) {
    let message = message.as_ref();
    eprintln!("{message}");

    let Ok(dir) = app.path().app_config_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(LOG_FILE);
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::write(&path, "");
        }
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

fn show_capture(app: &AppHandle, event: &str) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    } else {
        log_line(app, "main-окно не найдено — показать capture нельзя");
        return;
    }
    if let Err(e) = app.emit(event, ()) {
        log_line(app, format!("не удалось отправить событие {event}: {e}"));
    }
}

fn toggle_capture_window(app: &AppHandle, win: &WebviewWindow) {
    let visible = win.is_visible().unwrap_or(false);
    let focused = win.is_focused().unwrap_or(false);

    if visible && focused {
        let _ = win.hide();
    } else {
        let _ = win.show();
        let _ = win.set_focus();
        if let Err(e) = app.emit("capture-shown", ()) {
            log_line(app, format!("не удалось отправить capture-shown: {e}"));
        }
    }
}

/// Показывает окно по label или создаёт его. Если окно успели создать
/// параллельным вызовом, переиспользует существующее вместо ошибки.
fn open_window(
    app: &AppHandle,
    label: &str,
    title: &str,
    size: (f64, f64),
    min_size: (f64, f64),
) -> tauri::Result<WebviewWindow> {
    if let Some(win) = app.get_webview_window(label) {
        win.show()?;
        win.set_focus()?;
        return Ok(win);
    }

    let builder =
        tauri::WebviewWindowBuilder::new(app, label, tauri::WebviewUrl::App("index.html".into()))
            .title(title)
            .inner_size(size.0, size.1)
            .min_inner_size(min_size.0, min_size.1)
            .always_on_top(true)
            .center();

    match builder.build() {
        Ok(win) => {
            win.set_focus()?;
            Ok(win)
        }
        Err(err) => {
            if let Some(win) = app.get_webview_window(label) {
                win.show()?;
                win.set_focus()?;
                Ok(win)
            } else {
                Err(err)
            }
        }
    }
}

/// Окно настроек.
#[tauri::command]
fn open_settings(app: AppHandle) -> tauri::Result<()> {
    open_window(
        &app,
        "settings",
        "Настройки",
        (520.0, 600.0),
        (420.0, 420.0),
    )
    .map(|_| ())
}

/// Показывает окно календаря, создавая его при первом вызове.
#[tauri::command]
fn open_calendar(app: AppHandle) -> tauri::Result<()> {
    open_window(
        &app,
        "calendar",
        "Календарь",
        (720.0, 760.0),
        // уже, чем минимальная ширина года (572px + отступы): при сужении
        // появляется горизонтальный скролл, а не наезжающие месяцы
        (360.0, 480.0),
    )
    .map(|_| ())
}

/// Окно просмотра задачи: текст + лента комментариев.
/// Одно окно на задачу (label `task-{id}`), повторный вызов фокусирует его.
#[tauri::command]
fn open_task_window(app: AppHandle, task_id: i64) -> tauri::Result<()> {
    let label = format!("task-{task_id}");
    open_window(&app, &label, "Задача", (420.0, 560.0), (320.0, 360.0)).map(|_| ())
}

/// Разбирает комбинацию и требует хотя бы один модификатор —
/// иначе из JS можно было бы зарегистрировать перехват одиночной клавиши.
fn parse_shortcut(input: &str) -> Result<Shortcut, String> {
    let shortcut = Shortcut::from_str(input.trim())
        .map_err(|_| format!("Некорректная комбинация «{input}»"))?;
    let action_modifiers = Modifiers::SUPER | Modifiers::CONTROL | Modifiers::ALT;
    if !shortcut.mods.intersects(action_modifiers) {
        return Err(
            "Нужен хотя бы один из модификаторов ⌘, ⌃ или ⌥ (Shift сам по себе не подходит)"
                .to_string(),
        );
    }
    Ok(shortcut)
}

fn shortcut_in_use(
    app: &AppHandle,
    want: &Shortcut,
    excluding_capture: bool,
    excluding_new_task: bool,
) -> Result<bool, String> {
    if !excluding_capture {
        let state = app.state::<ActiveShortcut>();
        if state.is_registered() && &state.current()? == want {
            return Ok(true);
        }
    }
    if !excluding_new_task {
        let state = app.state::<ActiveNewTaskShortcut>();
        if state.is_registered() && &state.current()? == want {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Сначала регистрируем новую комбинацию и только после успеха снимаем
/// старую — при ошибке остаёмся на рабочей клавише.
fn swap_global_shortcut(app: &AppHandle, current: Shortcut, new: Shortcut) -> Result<(), String> {
    app.global_shortcut()
        .register(new)
        .map_err(|e| format!("Не удалось зарегистрировать «{new}»: {e}"))?;

    if let Err(e) = app.global_shortcut().unregister(current) {
        let _ = app.global_shortcut().unregister(new);
        return Err(format!(
            "Новая комбинация зарегистрирована, но не удалось снять старую «{current}»: {e}"
        ));
    }
    Ok(())
}

#[tauri::command]
fn get_shortcut(state: State<'_, ActiveShortcut>) -> Result<String, String> {
    state.current().map(|s| s.to_string())
}

#[tauri::command]
fn get_new_task_shortcut(state: State<'_, ActiveNewTaskShortcut>) -> Result<String, String> {
    state.current().map(|s| s.to_string())
}

/// Реально ли зарегистрирована комбинация вызова окна (а не просто сохранена).
#[tauri::command]
fn shortcut_registered(state: State<'_, ActiveShortcut>) -> bool {
    state.is_registered()
}

/// Реально ли зарегистрирована комбинация создания новой задачи.
#[tauri::command]
fn new_task_shortcut_registered(state: State<'_, ActiveNewTaskShortcut>) -> bool {
    state.is_registered()
}

/// Меняет глобальную комбинацию вызова окна.
#[tauri::command]
fn set_shortcut(app: AppHandle, shortcut: String) -> Result<String, String> {
    let new = parse_shortcut(&shortcut)?;
    let state = app.state::<ActiveShortcut>();
    let current = state.current()?;
    if current == new {
        if state.is_registered() {
            return Ok(new.to_string());
        }
        // Комбинация совпадает с сохранённой в state, но не зарегистрирована
        // (например, дефолт не удалось занять при старте): только регистрируем.
        app.global_shortcut()
            .register(new)
            .map_err(|e| format!("Не удалось зарегистрировать «{new}»: {e}"))?;
        state.mark_registered(new)?;
        return Ok(new.to_string());
    }
    if shortcut_in_use(&app, &new, true, false)? {
        return Err("Эта комбинация уже используется для создания новой задачи".into());
    }
    swap_global_shortcut(&app, current, new)?;
    state.mark_registered(new)?;
    Ok(new.to_string())
}

/// Меняет глобальную комбинацию создания новой задачи.
#[tauri::command]
fn set_new_task_shortcut(app: AppHandle, shortcut: String) -> Result<String, String> {
    let new = parse_shortcut(&shortcut)?;
    let state = app.state::<ActiveNewTaskShortcut>();
    let current = state.current()?;
    if current == new {
        if state.is_registered() {
            return Ok(new.to_string());
        }
        app.global_shortcut()
            .register(new)
            .map_err(|e| format!("Не удалось зарегистрировать «{new}»: {e}"))?;
        state.mark_registered(new)?;
        return Ok(new.to_string());
    }
    if shortcut_in_use(&app, &new, false, true)? {
        return Err("Эта комбинация уже используется для вызова окна".into());
    }
    swap_global_shortcut(&app, current, new)?;
    state.mark_registered(new)?;
    Ok(new.to_string())
}

/// Проверяет формат `YYYY-MM-DD` (защита от path traversal в имени бэкапа).
fn is_valid_day(day: &str) -> bool {
    let bytes = day.as_bytes();
    bytes.len() == 10
        && bytes.iter().enumerate().all(|(i, b)| {
            if i == 4 || i == 7 {
                *b == b'-'
            } else {
                b.is_ascii_digit()
            }
        })
}

fn backup_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Не удалось определить каталог данных: {e}"))?
        .join(BACKUP_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("Не удалось создать каталог бэкапов: {e}"))?;
    Ok(dir)
}

/// Готовит путь для ежедневного бэкапа. `Ok(None)` — бэкап за этот день уже есть.
/// Сам снимок делает фронтенд через `VACUUM INTO` (корректно для WAL).
#[tauri::command]
fn prepare_backup(app: AppHandle, day: String) -> Result<Option<String>, String> {
    if !is_valid_day(&day) {
        return Err("Некорректная дата бэкапа".to_string());
    }
    let target = backup_dir(&app)?.join(format!("{BACKUP_PREFIX}{day}.db"));
    if target.exists() {
        return Ok(None);
    }
    Ok(Some(target.to_string_lossy().to_string()))
}

/// Удаляет старые бэкапы, оставляя `keep` новейших (по умолчанию BACKUP_KEEP).
#[tauri::command]
fn prune_backups(app: AppHandle, keep: Option<u32>) -> Result<(), String> {
    let keep = keep.unwrap_or(BACKUP_KEEP);
    if keep == 0 {
        return Ok(());
    }
    let dir = backup_dir(&app)?;
    let entries = std::fs::read_dir(&dir)
        .map_err(|e| format!("Не удалось прочитать каталог бэкапов: {e}"))?;
    let mut files: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(BACKUP_PREFIX) && name.ends_with(".db"))
        })
        .collect();
    files.sort();
    let keep = keep as usize;
    if files.len() > keep {
        for old in &files[..files.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(())
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let new_task = MenuItem::with_id(app, "new-task", "Новая задача", true, None::<&str>)?;
    let calendar = MenuItem::with_id(app, "calendar", "Календарь", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Настройки", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Выход", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &new_task,
            &calendar,
            &settings,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let icon = app.default_window_icon().cloned().ok_or_else(|| {
        tauri::Error::InvalidIcon(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "window icon not found",
        ))
    })?;

    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "new-task" => show_capture(app, "new-task-shortcut"),
            "calendar" => {
                if let Err(e) = open_calendar(app.clone()) {
                    log_line(app, format!("не удалось открыть календарь: {e}"));
                }
            }
            "settings" => {
                if let Err(e) = open_settings(app.clone()) {
                    log_line(app, format!("не удалось открыть настройки: {e}"));
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .build(app)?;

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
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let capture_state = app.state::<ActiveShortcut>();
                    let is_capture = capture_state.is_registered()
                        && capture_state
                            .current()
                            .map(|s| &s == shortcut)
                            .unwrap_or(false);

                    let new_task_state = app.state::<ActiveNewTaskShortcut>();
                    let is_new_task = new_task_state.is_registered()
                        && new_task_state
                            .current()
                            .map(|s| &s == shortcut)
                            .unwrap_or(false);

                    if is_capture {
                        if let Some(win) = app.get_webview_window("main") {
                            toggle_capture_window(app, &win);
                        }
                    } else if is_new_task {
                        show_capture(app, "new-task-shortcut");
                    }
                })
                .build(),
        )
        .manage(ActiveShortcut::default())
        .manage(ActiveNewTaskShortcut::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let gs = app.handle().global_shortcut();
            let mut fallback_opened = false;

            let capture = default_shortcut();
            match gs.register(capture) {
                Ok(()) => {
                    if let Err(e) = app.state::<ActiveShortcut>().mark_registered(capture) {
                        log_line(app.handle(), format!("состояние шортката вызова окна: {e}"));
                    }
                }
                Err(e) => {
                    log_line(
                        app.handle(),
                        format!("не удалось зарегистрировать шорткат вызова окна: {e}"),
                    );
                    if !fallback_opened {
                        fallback_opened = true;
                        let _ = open_settings(app.handle().clone());
                    }
                }
            }

            let new_task = default_new_task_shortcut();
            match gs.register(new_task) {
                Ok(()) => {
                    if let Err(e) = app
                        .state::<ActiveNewTaskShortcut>()
                        .mark_registered(new_task)
                    {
                        log_line(
                            app.handle(),
                            format!("состояние шортката новой задачи: {e}"),
                        );
                    }
                }
                Err(e) => {
                    log_line(
                        app.handle(),
                        format!("не удалось зарегистрировать шорткат новой задачи: {e}"),
                    );
                    if !fallback_opened {
                        let _ = open_settings(app.handle().clone());
                    }
                }
            }

            if let Err(e) = build_tray(app.handle()) {
                log_line(app.handle(), format!("не удалось создать tray-меню: {e}"));
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // main-окно — единственный вход в capture; закрывать его нельзя.
            if window.label() == "main" {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_calendar,
            open_task_window,
            open_settings,
            get_shortcut,
            set_shortcut,
            get_new_task_shortcut,
            set_new_task_shortcut,
            shortcut_registered,
            new_task_shortcut_registered,
            prepare_backup,
            prune_backups
        ])
        .run(tauri::generate_context!())
    {
        eprintln!("error while running tauri application: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcuts_are_parseable_and_have_modifiers() {
        for value in [DEFAULT_SHORTCUT, DEFAULT_NEW_TASK_SHORTCUT] {
            let shortcut = Shortcut::from_str(value).expect("default shortcut must parse");
            assert!(!shortcut.mods.is_empty(), "{value} must have a modifier");
        }
    }

    #[test]
    fn shortcut_display_round_trips() {
        let shortcut = default_shortcut();
        let display = shortcut.to_string();
        let parsed = Shortcut::from_str(&display)
            .unwrap_or_else(|_| panic!("display form {display} must be parseable"));
        assert_eq!(parsed, shortcut);
    }

    #[test]
    fn migrations_are_ordered_unique_and_non_empty() {
        let mut previous = 0;
        for (version, description, sql) in MIGRATIONS {
            assert!(*version > previous, "migrations must be ordered by version");
            previous = *version;
            assert!(
                !description.is_empty(),
                "migration {version} needs description"
            );
            assert!(!sql.trim().is_empty(), "migration {version} is empty");
        }
    }

    #[test]
    fn parse_shortcut_requires_action_modifier() {
        assert!(parse_shortcut("Shift+D").is_err());
        assert!(parse_shortcut("Command+Shift+D").is_ok());
        assert!(parse_shortcut("Control+Alt+5").is_ok());
        assert!(parse_shortcut("CmdOrCtrl+Shift+N").is_ok());
    }

    #[test]
    fn backup_day_validation_rejects_path_traversal() {
        assert!(is_valid_day("2026-11-03"));
        assert!(is_valid_day("1999-01-01"));
        assert!(!is_valid_day("03.11.2026"));
        assert!(!is_valid_day("../../etc/passwd"));
        assert!(!is_valid_day("2026-11-3"));
        assert!(!is_valid_day(""));
    }
}
