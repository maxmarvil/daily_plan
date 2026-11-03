import Database from "@tauri-apps/plugin-sql";

let db: Database | null = null;

/**
 * Единый экземпляр БД на окно.
 *
 * Плагин создаёт sqlx-pool на каждый `Database.load()` (по пулу на окно);
 * sqlx включает FK для каждого соединения по умолчанию, поэтому PRAGMA ниже —
 * подстраховка, а не единственная гарантия (она применяется к одному
 * соединению пула).
 */
export async function getDb(): Promise<Database> {
  if (!db) {
    db = await Database.load("sqlite:daily-plan.db");
    await db.execute("PRAGMA foreign_keys = ON");
  }
  return db;
}
