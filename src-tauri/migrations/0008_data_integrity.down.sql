DROP TRIGGER IF EXISTS comments_original_only_update;
DROP TRIGGER IF EXISTS comments_original_only_insert;
DROP TRIGGER IF EXISTS tasks_no_delete;
DROP TRIGGER IF EXISTS tasks_immutable_update;
DROP TRIGGER IF EXISTS tasks_validate_update;
DROP TRIGGER IF EXISTS tasks_validate_insert;
DROP INDEX IF EXISTS idx_tasks_day_position;
