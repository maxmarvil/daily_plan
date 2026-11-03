#!/usr/bin/env bash
# Ручной откат SQLite-миграций до версии N (tauri-plugin-sql умеет только Up).
# Использует src-tauri/migrations/*.down.sql.
#
# usage: scripts/rollback.sh <target_version>
#   0 — откатить всё до пустой схемы
#
# Переопределение БД для тестов: DB_PATH=/tmp/x.db scripts/rollback.sh 5
set -euo pipefail

TARGET="${1:?usage: rollback.sh <target_version>}"
[[ "$TARGET" =~ ^[0-9]+$ ]] || { echo "target должен быть целым >= 0"; exit 1; }

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MIGRATIONS="$ROOT/src-tauri/migrations"
DB="${DB_PATH:-$HOME/Library/Application Support/app.dailyplan.desktop/daily-plan.db}"

command -v sqlite3 >/dev/null || { echo "нужен sqlite3"; exit 1; }
[[ -f "$DB" ]] || { echo "БД не найдена: $DB"; exit 1; }

CURRENT="$(sqlite3 "$DB" "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations;")"
(( TARGET <= CURRENT )) || { echo "target $TARGET > текущей $CURRENT"; exit 1; }
[[ "$CURRENT" == "$TARGET" ]] && { echo "уже на версии $TARGET"; exit 0; }

BACKUP="$DB.bak.$(date +%Y%m%d%H%M%S)"
cp "$DB" "$BACKUP"
echo "backup: $BACKUP"

for (( v = CURRENT; v > TARGET; v-- )); do
  file="$(find "$MIGRATIONS" -maxdepth 1 -name "$(printf '%04d' "$v")_"*.down.sql | head -1)"
  [[ -n "$file" ]] || { echo "нет down-скрипта для версии $v — остановлено"; exit 1; }
  echo "down $(basename "$file")"
  sqlite3 -bail "$DB" < "$file"
  sqlite3 "$DB" "DELETE FROM _sqlx_migrations WHERE version = $v;"
done

echo "готово: $CURRENT -> $TARGET"
