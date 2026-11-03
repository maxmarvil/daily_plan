-- одна и та же задача не может быть перенесена в один день дважды
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_transfer
ON tasks(source_id, day) WHERE source_id IS NOT NULL;
