# Сборка и распространение (macOS)

Инструкция: как собрать `Daily Plan.app` / `.dmg`, установить локально и
подготовить сборку к распространению.

## Что получится

`npm run tauri build` (или `npm run release`) собирает release-бинарь, фронтенд
и бандлы:

- `src-tauri/target/release/bundle/macos/Daily Plan.app`
- `src-tauri/target/release/bundle/dmg/Daily Plan_<version>_<arch>.dmg`
- `src-tauri/target/release/bundle/zip/Daily Plan_<version>_<arch>.zip`
  (создаёт `scripts/build-macos.sh`; запасной способ переноса, если DMG собрать
  не удалось)

Для DMG скрипт печатает SHA-256 (`shasum -a 256`).

Формат `targets` в `src-tauri/tauri.conf.json` — `["app", "dmg"]`.
Версия берётся из `tauri.conf.json` и должна совпадать с `package.json` и
`src-tauri/Cargo.toml` (это проверяет `scripts/build-macos.sh`).

## Предпосылки

- macOS с установленными **Xcode Command Line Tools**: `xcode-select --install`
  (полный Xcode нужен только для нотаризации/`notarytool`).
- Node.js 22 и npm (сборка идёт через `npm ci`).
- Rust: `rustup` + стабильный toolchain (`rust-version = "1.90"` в `Cargo.toml`).
- Для универсального бинаря (Intel + Apple Silicon) дополнительно:
  ```bash
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
  ```

## Локальная сборка

```bash
npm ci
npm run check         # svelte-check
npm test              # vitest
npm run tauri build   # или: npm run release / scripts/build-macos.sh
```

Скрипт `scripts/build-macos.sh` дополнительно проверяет совпадение версий и
печатает пути готовых артефактов:

```bash
scripts/build-macos.sh                 # текущая архитектура
scripts/build-macos.sh --target universal-apple-darwin   # universal (нужны оба target'а)
```

### Если DMG не собирается автоматически

Tauri оформляет окно DMG через Finder/AppleScript (фон, позиции иконок). Если
процессу не разрешена Automation (CI, песочница, агентское окружение), шаг
падает с ошибкой AppleEvent timeout (-1712). В этом случае
`scripts/build-macos.sh` автоматически вызывает `scripts/make-dmg.sh` и
собирает установочный DMG напрямую через `hdiutil`:

- внутри `.app`, ссылка на `/Applications`, иконка тома;
- без кастомного фона/раскладки Finder (функционально это обычный
  drag-to-Applications инсталлятор).

Отдельный запуск:

```bash
scripts/make-dmg.sh
```

Если же недоступен и `hdiutil` (жёсткая песочница), `.app` уже готов, а скрипт
дополнительно упакует его в `.zip` — приложение можно переносить и так.

Для штатного «красивого» DMG запускайте сборку в обычном Terminal на macOS и
разрешите Automation для Finder (Системные настройки → Конфиденциальность и
безопасность → Автоматизация).

### Установка на этом Mac

1. Откройте `.dmg` и перетащите **Daily Plan** в **Applications**.
2. Запустите из Applications (или через Spotlight).

Локально собранное приложение не имеет карантина, поэтому запускается без
предупреждений. Если `.dmg` был скачан/передан через интернет, macOS покажет
«приложение не может быть проверено». Для своей машины это лечится:

```bash
xattr -dr com.apple.quarantine "/Applications/Daily Plan.app"
```

(для распространения на чужие машины — только подпись + нотаризация, см. ниже).

### Где данные

- БД и бэкапы: `~/Library/Application Support/app.dailyplan.desktop/`
  (`daily-plan.db`, `backups/daily-plan-YYYY-MM-DD.db`).
- Обновление = замена `.app`; данные сохраняются.
- Удаление = удалить приложение; данные остаются, при необходимости удалите
  каталог вручную.
- Перед крупным обновлением схемы БД делайте копию каталога (или используйте
  `scripts/rollback.sh`).

## Подпись (Developer ID) и нотаризация

Нужен аккаунт **Apple Developer Program** (99 $/год). Бесплатный аккаунт даёт
подпись, но не нотаризацию — Gatekeeper всё равно предупредит.

### 1. Сертификат

Создайте сертификат **Developer ID Application** в
[Certificates, IDs & Profiles](https://developer.apple.com/account/resources/certificates/list),
скачайте `.cer`, установите в связку ключей **login**. Проверка:

```bash
security find-identity -v -p codesigning
# "Developer ID Application: ИМЯ (TEAMID)"
```

### 2. Локальная подпись + нотаризация

Tauri подписывает приложение, если задан `signingIdentity` (в
`bundle.macOS.signingIdentity`) или переменная `APPLE_SIGNING_IDENTITY`.
Если в окружении есть креды Apple, Tauri дополнительно отправляет бандл на
нотаризацию и пришивает тикет (`stapler`).

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: ИМЯ (TEAMID)"
export APPLE_ID="your@apple.id"
export APPLE_PASSWORD="xxxx-xxxx-xxxx-xxxx"   # app-specific password
export APPLE_TEAM_ID="TEAMID"
npm run tauri build
```

Вместо `APPLE_ID`/`APPLE_PASSWORD` можно использовать API-ключ App Store
Connect: `APPLE_API_KEY`, `APPLE_API_ISSUER`, `APPLE_API_KEY_PATH`
(подробности — в документации Tauri:
<https://v2.tauri.app/distribute/sign/macos/>).

`bundle.macOS.hardenedRuntime` включён по умолчанию (нужен для нотаризации).
Если понадобятся entitlements (например, доступ к защищённым каталогам или JIT),
добавьте файл и укажите `bundle.macOS.entitlements`. Текущему приложению
специальные entitlements не требуются.

### 3. Проверка подписи

```bash
codesign --verify --deep --strict --verbose=2 "src-tauri/target/release/bundle/macos/Daily Plan.app"
spctl -a -vvv -t install "src-tauri/target/release/bundle/macos/Daily Plan.app"
xcrun stapler validate "src-tauri/target/release/bundle/dmg/Daily Plan_0.2.0_aarch64.dmg"
```

### 4. CI (GitHub Actions)

Для подписи в CI сертификат кладут в secrets. Требуются:
`APPLE_CERTIFICATE` (base64 от `.p12`), `APPLE_CERTIFICATE_PASSWORD`,
`KEYCHAIN_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`,
`APPLE_TEAM_ID`. Официальный экшен `tauri-apps/tauri-action@v0` умеет собирать,
подписывать и нотаризовать; в матрице обычно `aarch64-apple-darwin` +
`x86_64-apple-darwin` (или `universal-apple-darwin`). Наш базовый CI
(`.github/workflows/ci.yml`) проверяет код и не собирает релиз — для релизов
добавляется отдельный workflow.

## Универсальный бинарь и DMG

```bash
rustup target add x86_64-apple-darwin   # если ещё не установлен
npm run tauri build -- --target universal-apple-darwin
```

Оформление DMG (фон, размер окна, позиции иконок) настраивается в
`bundle.macOS.dmg` (`background`, `windowSize`, `appPosition`,
`applicationFolderPosition`). Сейчас используются дефолты Tauri: окно 660×400,
приложение слева, папка Applications справа.

## Автообновление (опционально)

Если нужно обновлять приложение не вручную, подключается
`tauri-plugin-updater`: генерируется ключ (`npm run tauri signer generate`),
подпись артефактов и JSON-манифест выкладываются на статический хостинг
(или GitHub Releases), а в `tauri.conf.json` прописывается endpoint и
публичный ключ. Без подписи обновлений плагин работать не будет.

## Чек-лист релиза

- [ ] Версия одинаковая в `package.json`, `src-tauri/tauri.conf.json`,
      `src-tauri/Cargo.toml`; запись в `CHANGELOG.md`.
- [ ] `npm ci && npm test && npm run check && npm run build` — зелёные;
      `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`.
- [ ] `scripts/rollback.sh` актуален, новая миграция зарегистрирована в
      `MIGRATIONS` и проверена на копии реальной БД.
- [ ] Иконки обновлены (`npm run tauri icon app-icon.png`), `bundle.icon` полон.
- [ ] Проверен первый запуск на «чистой» машине (или чистом профиле):
      скрытое окно, шорткаты, глобальный хоткей, бэкап БД.
- [ ] Подпись и нотаризация (для распространения), `spctl`/`stapler` — ok.
- [ ] `.dmg` проверен: перетаскивание в Applications, запуск, данные на месте;
      при недоступной Automation у Finder использован `scripts/make-dmg.sh`.
