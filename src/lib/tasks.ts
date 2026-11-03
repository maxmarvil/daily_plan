import type Database from "@tauri-apps/plugin-sql";

/** Строка задачи для списков (capture/месяц). Перенос отдаёт предпросмотр оригинала. */
export type TaskRow = {
  id: number;
  day: string;
  position: number;
  text: string;
  /** Для переноса — done оригинала; для обычной задачи — её собственный. */
  is_done: number;
  source_id: number | null;
  /** День оригинала (только у переноса). */
  source_day: string | null;
  /** 1, если текст оригинала обрезан до 40 символов. */
  truncated: number;
  /** Число комментариев у оригинала. */
  comments: number;
  /** Число переносов этой (оригинальной) задачи. */
  transfers: number;
};

/** Длина предпросмотра переноса (CONCEPT: первые ~40 символов). */
export const PREVIEW_LIMIT = 40;

/** Одна строка предпросмотра: схлопываем переводы строк, обрезаем до 40. */
const PREVIEW_SQL = `substr(replace(replace(COALESCE(s.text, ''), char(10), ' '), char(13), ' '), 1, ${PREVIEW_LIMIT})`;

const TASK_COLUMNS = `
  t.id,
  t.day,
  t.position,
  t.source_id,
  s.day AS source_day,
  COALESCE(s.is_done, t.is_done) AS is_done,
  CASE WHEN t.source_id IS NULL
            THEN t.text
            ELSE ${PREVIEW_SQL}
       END AS text,
  CASE WHEN t.source_id IS NOT NULL
            AND length(COALESCE(s.text, '')) > ${PREVIEW_LIMIT}
       THEN 1 ELSE 0
  END AS truncated,
  (SELECT COUNT(*) FROM comments c
    WHERE c.task_id = COALESCE(t.source_id, t.id)) AS comments,
  (SELECT COUNT(*) FROM tasks w WHERE w.source_id = t.id) AS transfers
`;

const TASK_FROM = `
  FROM tasks t
  LEFT JOIN tasks s ON s.id = t.source_id
`;

/** Задачи одного дня (capture) — с предпросмотром переносов и бейджами. */
export async function loadDayTasks(
  db: Database,
  day: string,
): Promise<TaskRow[]> {
  return db.select<TaskRow[]>(
    `SELECT ${TASK_COLUMNS} ${TASK_FROM}
     WHERE t.day = $1
     ORDER BY t.position`,
    [day],
  );
}

/** Задачи диапазона дней (режим «Месяц»), новые дни сверху. */
export async function loadMonthTasks(
  db: Database,
  from: string,
  to: string,
): Promise<TaskRow[]> {
  return db.select<TaskRow[]>(
    `SELECT ${TASK_COLUMNS} ${TASK_FROM}
     WHERE t.day BETWEEN $1 AND $2
     ORDER BY t.day DESC, t.position`,
    [from, to],
  );
}

/** Счётчики задач по дням диапазона — для зимы/пикера переноса. */
export async function loadRangeCounts(
  db: Database,
  from: string,
  to: string,
): Promise<Record<string, number>> {
  const rows = await db.select<{ day: string; n: number }[]>(
    `SELECT day, COUNT(*) AS n
       FROM tasks
      WHERE day BETWEEN $1 AND $2
      GROUP BY day`,
    [from, to],
  );
  return Object.fromEntries(rows.map((row) => [row.day, row.n]));
}

/** Дни, куда эта задача уже перенесена. */
export async function loadTransferredDays(
  db: Database,
  sourceId: number,
): Promise<string[]> {
  const rows = await db.select<{ day: string }[]>(
    `SELECT day FROM tasks WHERE source_id = $1 ORDER BY day`,
    [sourceId],
  );
  return rows.map((row) => row.day);
}

/** Сырая задача для окна просмотра (без предпросмотра). */
export async function loadRawTask(
  db: Database,
  id: number,
): Promise<{ id: number; day: string; text: string; source_id: number | null; is_done: number } | null> {
  const rows = await db.select<
    { id: number; day: string; text: string; source_id: number | null; is_done: number }[]
  >(`SELECT id, day, text, source_id, is_done FROM tasks WHERE id = $1`, [id]);
  return rows[0] ?? null;
}

export async function loadComments(
  db: Database,
  taskId: number,
): Promise<{ id: number; text: string; created_at: string }[]> {
  return db.select(
    `SELECT id, text, created_at FROM comments
      WHERE task_id = $1 ORDER BY created_at`,
    [taskId],
  );
}

/**
 * Повторяет запись при коллизии уникального индекса (например, гонка
 * за `position` в одном дне): запрос каждый раз пересчитывает MAX+1.
 */
async function executeWithRetry(
  db: Database,
  query: string,
  values: unknown[],
  attempts = 3,
): Promise<number> {
  let lastError: unknown;
  for (let attempt = 0; attempt < attempts; attempt++) {
    try {
      const result = await db.execute(query, values);
      return result.rowsAffected;
    } catch (error) {
      lastError = error;
      const message = String(error);
      // UNIQUE/constraint — можно попробовать ещё раз; прочее — сразу наверх.
      if (!/UNIQUE|constraint/i.test(message)) throw error;
    }
  }
  throw lastError;
}

const INSERT_TASK_SQL = `
  INSERT INTO tasks (day, position, text, created_at, updated_at)
  SELECT $1,
         (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE day = $1),
         $2, $3, $3
`;

/** Вставляет задачу в день; возвращает rowsAffected (1 или 0). */
export async function insertTask(
  db: Database,
  day: string,
  text: string,
): Promise<number> {
  return executeWithRetry(db, INSERT_TASK_SQL, [day, text, new Date().toISOString()]);
}

export async function updateTaskText(
  db: Database,
  taskId: number,
  text: string,
): Promise<void> {
  await db.execute(`UPDATE tasks SET text = $1, updated_at = $2 WHERE id = $3`, [
    text,
    new Date().toISOString(),
    taskId,
  ]);
}

/** done — свойство оригинала: у переноса переключаем состояние оригинала. */
export async function setTaskDone(
  db: Database,
  task: Pick<TaskRow, "id" | "source_id" | "is_done">,
): Promise<void> {
  const targetId = task.source_id ?? task.id;
  await db.execute(`UPDATE tasks SET is_done = $1, updated_at = $2 WHERE id = $3`, [
    task.is_done ? 0 : 1,
    new Date().toISOString(),
    targetId,
  ]);
}

const INSERT_TRANSFER_SQL = `
  INSERT INTO tasks (day, position, text, source_id, created_at, updated_at)
  SELECT $1,
         (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE day = $1),
         '', $2, $3, $3
  WHERE NOT EXISTS (
    SELECT 1 FROM tasks WHERE source_id = $2 AND day = $1
  )
`;

/** Создаёт запись-перенос; 0 строк — такой перенос уже есть. */
export async function insertTransfer(
  db: Database,
  targetDay: string,
  sourceId: number,
): Promise<number> {
  return executeWithRetry(db, INSERT_TRANSFER_SQL, [
    targetDay,
    sourceId,
    new Date().toISOString(),
  ]);
}

export async function insertComment(
  db: Database,
  taskId: number,
  text: string,
): Promise<void> {
  await db.execute(
    `INSERT INTO comments (task_id, text, created_at) VALUES ($1, $2, $3)`,
    [taskId, text, new Date().toISOString()],
  );
}
