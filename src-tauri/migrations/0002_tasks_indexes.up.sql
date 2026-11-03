CREATE INDEX IF NOT EXISTS idx_tasks_day ON tasks(day, position);
CREATE INDEX IF NOT EXISTS idx_tasks_source ON tasks(source_id);
