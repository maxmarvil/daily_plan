#!/usr/bin/env bash
# Создаёт установочный DMG из уже собранного .app напрямую через hdiutil.
#
# Нужен как фолбэк: Tauri-сборщик DMG стилизует окно через Finder/AppleScript
# и падает, если процессу не разрешена Automation (CI, песочница, агент).
# Здесь нет кастомного фона, но есть .app, ссылка на /Applications и иконка тома.
#
# usage: scripts/make-dmg.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p 'require("./package.json").version')"
case "$(uname -m)" in
  arm64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *) ARCH="$(uname -m)" ;;
esac

APP="src-tauri/target/release/bundle/macos/Daily Plan.app"
OUT_DIR="src-tauri/target/release/bundle/dmg"
OUT="$OUT_DIR/Daily Plan_${VERSION}_${ARCH}.dmg"

if [ ! -d "$APP" ]; then
  echo "Нет собранного приложения: $APP" >&2
  echo "Сначала: npm run tauri build -- --bundles app" >&2
  exit 1
fi
command -v hdiutil >/dev/null 2>&1 || {
  echo "В PATH нет hdiutil (нужен macOS)." >&2
  exit 1
}

STAGING="$(mktemp -d "${TMPDIR:-/tmp}/daily-plan-dmg.XXXXXX")"
MOUNT="$(mktemp -d "${TMPDIR:-/tmp}/daily-plan-mnt.XXXXXX")"
cleanup() {
  hdiutil detach "$MOUNT" >/dev/null 2>&1 || true
  rm -rf "$STAGING" "$MOUNT"
}
trap cleanup EXIT

# ditto сохраняет права/подпись/ресурсы .app
ditto "$APP" "$STAGING/Daily Plan.app"
ln -s /Applications "$STAGING/Applications"
if [ -f "src-tauri/icons/icon.icns" ]; then
  cp "src-tauri/icons/icon.icns" "$STAGING/.VolumeIcon.icns"
  if command -v SetFile >/dev/null 2>&1; then
    SetFile -a C "$STAGING" || true
  fi
fi

mkdir -p "$OUT_DIR"
rm -f "$OUT"

echo "== hdiutil create =="
hdiutil create -volname "Daily Plan" -srcfolder "$STAGING" -ov -format UDZO "$OUT"

echo "== verify =="
hdiutil verify "$OUT"

echo "== smoke mount =="
hdiutil attach -nobrowse -readonly -mountpoint "$MOUNT" "$OUT" >/dev/null
test -d "$MOUNT/Daily Plan.app"
test -L "$MOUNT/Applications"
hdiutil detach "$MOUNT" >/dev/null

echo
echo "DMG: $ROOT/$OUT"
ls -lh "$OUT"
shasum -a 256 "$OUT"
