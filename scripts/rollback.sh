#!/usr/bin/env bash
# Ручной откат SQLite-миграций до версии N.
# tauri-plugin-sql умеет применять только Up-миграции, поэтому Down
# применяются этим скриптом вручную из src-tauri/migrations/*.down.sql.
#
# usage: scripts/rollback.sh <target_version>
#   0 — откатить всё до пустой схемы
#
# Переопределение БД (для тестов на копии): DB_PATH=/tmp/x.db scripts/rollback.sh 6
# Приложение должно быть закрыто: pgrep находит "daily-plan"/"Daily Plan".
# Аварийный обход (на свой риск): FORCE=1 scripts/rollback.sh 6
#
# Безопасность:
#   * бэкап делается `sqlite3 .backup` (БД в WAL: cp может потерять
#     закоммиченные данные из -wal), затем проверяется integrity_check копии;
#   * preflight: все нужные .down.sql проверяются ДО любых изменений;
#   * каждая Down-миграция и удаление строки _sqlx_migrations выполняются
#     в одной транзакции; при ошибке изменения откатываются.
set -euo pipefail

usage() {
  cat >&2 <<'EOF'
usage: scripts/rollback.sh <target_version>

  target_version  целое >= 0; 0 — откатить все миграции
  DB_PATH         путь к БД (по умолчанию — БД приложения Daily Plan)
  FORCE=1         игнорировать проверку, что приложение запущено (опасно!)
EOF
}

case "${1:-}" in
  -h|--help) usage; exit 0 ;;
  "") usage; exit 1 ;;
esac

TARGET="$1"
[[ "$TARGET" =~ ^[0-9]+$ ]] || { echo "ошибка: target должен быть целым >= 0, получено: $TARGET" >&2; exit 1; }
TARGET=$((10#$TARGET)) # без этого "08" bash считает невалидным восьмеричным

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATIONS="$ROOT/src-tauri/migrations"
DB="${DB_PATH:-$HOME/Library/Application Support/app.dailyplan.desktop/daily-plan.db}"
SQLITE=(sqlite3 -bail -cmd ".timeout 5000")

command -v sqlite3 >/dev/null 2>&1 || { echo "ошибка: в PATH нет sqlite3" >&2; exit 1; }
[[ -d "$MIGRATIONS" ]] || { echo "ошибка: не найдена папка миграций: $MIGRATIONS" >&2; exit 1; }
[[ -f "$DB" ]] || { echo "ошибка: БД не найдена: $DB" >&2; exit 1; }
[[ -r "$DB" ]] || { echo "ошибка: БД недоступна для чтения: $DB" >&2; exit 1; }
[[ -w "$DB" ]] || { echo "ошибка: нет прав на запись в БД: $DB" >&2; exit 1; }
DB_DIR="$(dirname "$DB")"
[[ -w "$DB_DIR" ]] || { echo "ошибка: нет прав на запись в каталог БД (нужен для -wal и бэкапа): $DB_DIR" >&2; exit 1; }

# _sqlx_migrations создаётся tauri-plugin-sql при первом запуске.
TABLE_EXISTS=$("${SQLITE[@]}" "$DB" "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='_sqlx_migrations';")
[[ "$TABLE_EXISTS" == "1" ]] || { echo "ошибка: в БД нет таблицы _sqlx_migrations: $DB" >&2; exit 1; }

CURRENT=$("${SQLITE[@]}" "$DB" "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations;")
[[ "$CURRENT" =~ ^[0-9]+$ ]] || { echo "ошибка: не удалось определить текущую версию миграций" >&2; exit 1; }

(( TARGET <= CURRENT )) || { echo "ошибка: target $TARGET больше текущей версии схемы $CURRENT (откат только вниз)" >&2; exit 1; }
if (( TARGET == CURRENT )); then
  echo "Уже на версии $TARGET — делать нечего."
  exit 0
fi

# --- приложение не должно быть запущено: иначе оно держит БД и может
#     применить откатанные Up-миграции прямо во время отката.
app_running() {
  pgrep -x daily-plan >/dev/null 2>&1 && return 0
  pgrep -x "Daily Plan" >/dev/null 2>&1 && return 0
  return 1
}
if app_running; then
  if [[ "${FORCE:-0}" != "1" ]]; then
    cat >&2 <<EOF
ошибка: Daily Plan запущен (pgrep daily-plan / "Daily Plan").
Закрой приложение и повтори. Иначе приложение продолжит писать в БД и
следующий его запуск снова применит откатанные Up-миграции.
Аварийный обход (на свой риск): FORCE=1 $0 $TARGET
EOF
    exit 1
  fi
  echo "ВНИМАНИЕ: FORCE=1 — приложение запущено, откат выполняется на свой риск." >&2
fi

# --- preflight: все Down-файлы должны существовать ДО изменений.
down_file_for() {
  local v="$1"
  find "$MIGRATIONS" -maxdepth 1 -type f \
    -name "$(printf '%04d' "$v")_*.down.sql" -print -quit 2>/dev/null
}

declare -a FILES=()
for (( v = CURRENT; v > TARGET; v-- )); do
  f="$(down_file_for "$v")"
  if [[ -z "$f" ]]; then
    echo "ошибка: нет down-миграции для версии $v в $MIGRATIONS" >&2
    echo "        preflight провален — БД не изменялась." >&2
    exit 1
  fi
  FILES+=("$f")
done
echo "preflight: найдено down-файлов: ${#FILES[@]} ($CURRENT -> $TARGET)"

# --- бэкап через .backup (не cp: БД в WAL).
sql_quote() { printf '%s' "$1" | sed "s/'/''/g"; }
BACKUP="$DB.bak.$(date +%Y%m%d%H%M%S)"
n=1
while [[ -e "$BACKUP" ]]; do BACKUP="$DB.bak.$(date +%Y%m%d%H%M%S).$n"; n=$((n + 1)); done

"${SQLITE[@]}" "$DB" ".backup '$(sql_quote "$BACKUP")'"
[[ -f "$BACKUP" ]] || { echo "ошибка: бэкап не создан: $BACKUP" >&2; exit 1; }
[[ "$("${SQLITE[@]}" "$BACKUP" "PRAGMA integrity_check;")" == "ok" ]] || {
  echo "ошибка: integrity_check бэкапа не ok — откат остановлен, БД не изменялась: $BACKUP" >&2
  exit 1
}
# Бэкап наследует WAL-режим: сбрасываем WAL в основной файл, чтобы копия
# была самодостаточной (восстанавливать — только после удаления старых -wal/-shm).
"${SQLITE[@]}" "$BACKUP" "PRAGMA wal_checkpoint(TRUNCATE);" >/dev/null
rm -f "$BACKUP-wal" "$BACKUP-shm"
echo "бэкап: $BACKUP (integrity ok)"

# --- применяем Down-миграции: down + удаление записи _sqlx_migrations
#     в одной транзакции; sqlite3 -bail прервёт выполнение на первой ошибке,
#     незавершённая транзакция откатится при закрытии соединения.
for (( i = 0; i < ${#FILES[@]}; i++ )); do
  v=$(( CURRENT - i ))
  f="${FILES[$i]}"
  echo "down $v: $(basename "$f")"
  {
    echo "BEGIN;"
    cat "$f"
    echo "DELETE FROM _sqlx_migrations WHERE version = $v;"
    echo "COMMIT;"
  } | "${SQLITE[@]}" "$DB"
done

# --- проверка результата.
NEW_CURRENT=$("${SQLITE[@]}" "$DB" "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations;")
[[ "$NEW_CURRENT" == "$TARGET" ]] || {
  echo "ошибка: после отката версия $NEW_CURRENT, ожидалась $TARGET. Бэкап: $BACKUP" >&2
  exit 1
}
INTEGRITY=$("${SQLITE[@]}" "$DB" "PRAGMA integrity_check;")
[[ "$INTEGRITY" == "ok" ]] || {
  echo "ошибка: integrity_check после отката: $INTEGRITY. Бэкап: $BACKUP" >&2
  exit 1
}

cat <<EOF

Готово: схема откачена $CURRENT -> $TARGET (integrity ok).
Бэкап: $BACKUP

ВАЖНО: следующий запуск Daily Plan (tauri-plugin-sql выполняет только Up)
снова применит откатанные Up-миграции и вернёт схему к последней версии.
Чтобы закрепить откат, не запускай приложение; чтобы вернуть данные —
восстанови БД из бэкапа: удали "$DB-wal"/"$DB-shm" и скопируй бэкап на место БД.
EOF
