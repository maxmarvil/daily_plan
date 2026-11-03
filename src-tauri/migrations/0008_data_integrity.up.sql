-- 0008: инварианты данных на уровне БД.
--
-- Внимание: миграция не исправляет уже испорченные данные. Если в БД есть
-- дубли (day, position) или строки, не проходящие проверки, создание
-- уникального индекса/триггеров не спасёт: индекс упадёт, и приложение не
-- стартует. На чистой БД (все данные создавались приложением) миграция
-- проходит; так и задумано — молча править историю нельзя.

-- 1) Номер задачи уникален внутри дня.
CREATE UNIQUE INDEX IF NOT EXISTS idx_tasks_day_position ON tasks(day, position);

-- 2) Строка задачи валидна: формат дня, непустой текст у обычной задачи,
--    пустой текст у переноса, без self-reference и переноса-на-перенос.
CREATE TRIGGER IF NOT EXISTS tasks_validate_insert
BEFORE INSERT ON tasks
WHEN NEW.day NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
  OR date(NEW.day) IS NOT NEW.day
  OR NEW.position < 1
  OR NEW.is_done NOT IN (0, 1)
  OR (NEW.source_id IS NULL AND trim(NEW.text) = '')
  OR (NEW.source_id IS NOT NULL AND trim(NEW.text) <> '')
  OR (NEW.source_id IS NOT NULL AND NEW.source_id = NEW.id)
  OR (NEW.source_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM tasks WHERE id = NEW.source_id AND source_id IS NOT NULL))
BEGIN
  SELECT RAISE(ABORT, 'invalid task row');
END;

CREATE TRIGGER IF NOT EXISTS tasks_validate_update
BEFORE UPDATE ON tasks
WHEN NEW.day NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
  OR date(NEW.day) IS NOT NEW.day
  OR NEW.position < 1
  OR NEW.is_done NOT IN (0, 1)
  OR (NEW.source_id IS NULL AND trim(NEW.text) = '')
  OR (NEW.source_id IS NOT NULL AND trim(NEW.text) <> '')
  OR (NEW.source_id IS NOT NULL AND NEW.source_id = NEW.id)
  OR (NEW.source_id IS NOT NULL AND EXISTS (
        SELECT 1 FROM tasks WHERE id = NEW.source_id AND source_id IS NOT NULL))
BEGIN
  SELECT RAISE(ABORT, 'invalid task row');
END;

-- 3) День, время создания и источник задаются при создании и не меняются.
CREATE TRIGGER IF NOT EXISTS tasks_immutable_update
BEFORE UPDATE ON tasks
WHEN NEW.day <> OLD.day
  OR NEW.created_at <> OLD.created_at
  OR NEW.source_id IS NOT OLD.source_id
BEGIN
  SELECT RAISE(ABORT, 'task day/created_at/source_id are immutable');
END;

-- 4) Задачи не удаляются — история неизменяема.
CREATE TRIGGER IF NOT EXISTS tasks_no_delete
BEFORE DELETE ON tasks
BEGIN
  SELECT RAISE(ABORT, 'tasks cannot be deleted');
END;

-- 5) Комментарии можно добавлять только к оригиналу задачи.
CREATE TRIGGER IF NOT EXISTS comments_original_only_insert
BEFORE INSERT ON comments
WHEN EXISTS (SELECT 1 FROM tasks WHERE id = NEW.task_id AND source_id IS NOT NULL)
BEGIN
  SELECT RAISE(ABORT, 'comments must reference the original task');
END;

CREATE TRIGGER IF NOT EXISTS comments_original_only_update
BEFORE UPDATE ON comments
WHEN EXISTS (SELECT 1 FROM tasks WHERE id = NEW.task_id AND source_id IS NOT NULL)
BEGIN
  SELECT RAISE(ABORT, 'comments must reference the original task');
END;
