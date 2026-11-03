import Database from "@tauri-apps/plugin-sql";

let db: Database | null = null;

/** Единый экземпляр БД с включёнными внешними ключами. */
export async function getDb(): Promise<Database> {
  if (!db) {
    db = await Database.load("sqlite:daily-plan.db");
    // SQLite не включает проверку FK по умолчанию; без этого
    // ON DELETE CASCADE и REFERENCES не работают.
    await db.execute("PRAGMA foreign_keys = ON");
  }
  return db;
}
