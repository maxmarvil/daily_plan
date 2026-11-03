#!/usr/bin/env bash
# Сборка macOS-релиза Daily Plan (.app + .dmg, при недоступном Finder —
# DMG напрямую через hdiutil; при недоступном hdiutil — .zip).
#
# usage:
#   scripts/build-macos.sh                          # текущая архитектура
#   scripts/build-macos.sh --target universal-apple-darwin
#   scripts/build-macos.sh --bundles app            # только .app
#
# Перед сборкой проверяет совпадение версий и гоняет npm run check + npm test.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(node -p 'require("./package.json").version')"
case "$(uname -m)" in
  arm64) ARCH="aarch64" ;;
  x86_64) ARCH="x64" ;;
  *) ARCH="$(uname -m)" ;;
esac

echo "== Проверка версий =="
node <<'NODE'
const fs = require("fs");
const pkg = JSON.parse(fs.readFileSync("package.json", "utf8"));
const conf = JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json", "utf8"));
const cargo = fs.readFileSync("src-tauri/Cargo.toml", "utf8");
const cargoVersion = cargo.match(/^version = "([^"]+)"/m)?.[1];
if (!cargoVersion) {
  console.error("Не удалось прочитать version из src-tauri/Cargo.toml");
  process.exit(1);
}
if (pkg.version !== conf.version || pkg.version !== cargoVersion) {
  console.error(
    `Версии не совпадают: package.json=${pkg.version}, tauri.conf.json=${conf.version}, Cargo.toml=${cargoVersion}`,
  );
  process.exit(1);
}
console.log(`версия ${pkg.version}`);
NODE

echo "== Проверки =="
npm run check
npm test

echo "== Сборка =="
set +e
npx tauri build "$@"
BUILD_STATUS=$?
set -e

echo
echo "== Артефакты =="
shopt -s nullglob
app_path=""
dmg_path=""

for app in src-tauri/target/release/bundle/macos/*.app; do
  [ -e "$app" ] || continue
  app_path="$app"
done

for dmg in src-tauri/target/release/bundle/dmg/*.dmg; do
  [ -e "$dmg" ] || continue
  dmg_path="$dmg"
done

# Tauri-сборщик DMG стилизует окно через Finder/AppleScript. Если Automation
# недоступна (CI, песочница, агент) — шаг падает; собираем DMG напрямую.
if [ -z "$dmg_path" ] && [ -n "$app_path" ] && command -v hdiutil >/dev/null 2>&1; then
  echo
  echo "Штатный DMG от Tauri не получен — собираю через hdiutil (scripts/make-dmg.sh)…"
  if scripts/make-dmg.sh; then
    BUILD_STATUS=0
    for dmg in src-tauri/target/release/bundle/dmg/*.dmg; do
      [ -e "$dmg" ] || continue
      dmg_path="$dmg"
    done
  fi
fi

found=0
if [ -n "$app_path" ]; then
  echo "APP:  $ROOT/$app_path"
  found=1
fi
if [ -n "$dmg_path" ]; then
  echo "DMG:  $ROOT/$dmg_path"
  found=1
fi

# ZIP полезен и как артефакт переноса, и как запасной вариант без hdiutil.
if [ -n "$app_path" ]; then
  zip_dir="src-tauri/target/release/bundle/zip"
  zip_path="$zip_dir/Daily Plan_${VERSION}_${ARCH}.zip"
  mkdir -p "$zip_dir"
  ditto -c -k --sequesterRsrc --keepParent "$app_path" "$zip_path"
  echo "ZIP:  $ROOT/$zip_path"
fi

if [ "$found" -eq 0 ]; then
  echo "Бандлы не найдены — проверьте вывод сборки выше."
  exit 1
fi

if [ "$BUILD_STATUS" -ne 0 ]; then
  echo
  echo "Сборка завершилась с ошибкой. .app/.zip могут быть готовы;"
  echo "DMG требует hdiutil — запустите сборку в обычном Terminal."
  exit "$BUILD_STATUS"
fi
