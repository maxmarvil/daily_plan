import type Database from "@tauri-apps/plugin-sql";

export const SETTING_CAPTURE_SHORTCUT = "capture_shortcut";
export const SETTING_NEW_TASK_SHORTCUT = "new_task_shortcut";

/** Читает значение из таблицы settings; null, если ключа нет. */
export async function getSetting(
  db: Database,
  key: string,
): Promise<string | null> {
  const rows = await db.select<{ value: string }[]>(
    `SELECT value FROM settings WHERE key = $1`,
    [key],
  );
  return rows[0]?.value ?? null;
}

/** Сохраняет значение в settings (upsert по ключу). */
export async function setSetting(
  db: Database,
  key: string,
  value: string,
): Promise<void> {
  await db.execute(
    `INSERT INTO settings (key, value) VALUES ($1, $2)
     ON CONFLICT(key) DO UPDATE SET value = $3`,
    [key, value, value],
  );
}
