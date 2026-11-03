-- задачи: без заголовков, нумерация = порядковый номер внутри дня
CREATE TABLE IF NOT EXISTS tasks (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  day         TEXT    NOT NULL,
  position    INTEGER NOT NULL,
  text        TEXT    NOT NULL DEFAULT '',
  source_id   INTEGER REFERENCES tasks(id),
  is_transfer INTEGER GENERATED ALWAYS AS (source_id IS NOT NULL) VIRTUAL,
  created_at  TEXT    NOT NULL,
  updated_at  TEXT    NOT NULL
);
