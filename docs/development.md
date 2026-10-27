# Установка, разработка и публикация

[Индекс](README.md) · [Архитектура](architecture.md) · [CONTRIBUTING](../CONTRIBUTING.md)

## Поддерживаемая среда

| Компонент | Состояние |
|---|---|
| Нативная целевая платформа | macOS Apple Silicon, Metal; проверено локально |
| Среда проверки документации 27.10.2026 | macOS 26.6.2, arm64, Rust 1.99.1 |
| Rust | Edition 2024; в workspace объявлено `rust-version=1.89`; отдельная проверка минимального toolchain не выполнена |
| GUI | Bevy 0.18.1, bevy_egui 0.39.1; конкретное разрешение фиксируется Cargo.lock |
| Сборка | Не более двух jobs по `.cargo/config.toml`; dev/test без debug symbols и incremental |
| Другие ОС | CLI/ядро не зависят от GUI, но прохождение сборки/визуальных проверок Linux/Windows не заявляется |
| Сеть | Нужна для первого получения зависимостей и внешних ссылок; приложению в обычной работе не нужна |

Не интерпретируйте версию `rust-version` как подтверждённый CI-диапазон. Для нового окружения используйте актуальный stable; при проблеме воспроизведите на toolchain журнала [verification.md](verification.md).

## Обычная установка

Установите Rust/rustup подходящим для своей системы способом, Xcode Command Line Tools и компоненты:

```sh
xcode-select --install
rustup component add rustfmt clippy
git clone https://github.com/arsenysmorodin/brain-engine.git
cd brain-engine
cargo fetch --locked
cargo run -p neuro-app --locked
```

Если инструменты уже установлены, повторять их установку не требуется. `cargo run` без `-p neuro-app` не означает запуск GUI: приложение не входит в `default-members`.

После первой успешной сборки:

```sh
cargo run -p neuro-app --locked --offline
./target/debug/neuro-app
```

Встроенные `data/`, геометрия и шрифты включаются в бинарный файл. Обычный запуск не читает исходные PDF и не требует локального кеша импорта атласа.

## Изолированный toolchain этой рабочей копии

`./scripts/cargo-local` устанавливает переменные для `.local/cargo` и `.local/rustup`, затем вызывает уже существующий там Cargo. `.local/` исключён из Git: в новом клоне этот helper **не является bootstrap-установщиком**.

Если изолированный toolchain уже подготовлен:

```sh
./scripts/cargo-local test --workspace --locked
./scripts/cargo-local run -p neuro-app --locked --offline
```

Для запуска macOS-packager через тот же toolchain:

```sh
env PATH="$PWD/.local/cargo/bin:$PATH" \
  RUSTUP_HOME="$PWD/.local/rustup" CARGO_HOME="$PWD/.local/cargo" \
  RUSTUP_TOOLCHAIN=stable ./scripts/macos-app
```

Helper сборки `.app` предпочитает `cargo` из PATH, иначе вызывает `cargo-local`. Поэтому при нескольких Rust-установках явно выбирайте нужную среду.

## Рабочий цикл

```sh
cargo check -p neuro-domain -p neuro-engine -p neuro-cli --locked
cargo run -p neuro-cli --locked -- run scenarios/competition.json
cargo run -p neuro-app --locked
```

Изменения `include_str!`/`include_bytes!` в данных/ресурсах требуют пересборки. `output/` и `target/` — генерируемые результаты. Не редактируйте вручную встроенный отчёт подбора вместо изменения данных/алгоритма и воспроизводимого пересчёта.

## Обязательные проверки

Из корня репозитория, на неизменённом Cargo.lock:

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked
cargo doc --workspace --no-deps --locked
```

Генерируемая API-документация — `target/doc/neuro_engine/index.html` и `target/doc/neuro_domain/index.html`. В репозиторий её не коммитят. Новый численный модуль требует подходящих аналитических/численных тестов, а не только теста «получилось конечное число».

Изменение формул требует ревизии модели, уравнений/единиц/ограничений в документации и проверки старых сценариев. Не повышайте число build jobs и не добавляйте Bevy features без необходимости. Тесты не заменяют визуальный прогон.

## Нативные smoke-проверки

После `cargo build --workspace --locked` запускайте команды по одной. Они открывают отдельное окно и завершаются автоматически; артефакты сохраняются в указанный каталог. Используйте новый каталог, чтобы не принять старый маркер за успех.

| Команда после `./target/debug/neuro-app` | Покрытие / конечный маркер |
|---|---|
| `--smoke-test output/smoke-full` | Полный сценарий от 0 до 1060 кадров; `launcher-passed.txt` и предшествующие маркеры |
| `--smoke-test output/smoke-launcher --launcher-only` | Форма, отдельные M/T, смесь, отмена/ошибки; `launcher-passed.txt` |
| `--smoke-test output/smoke-circulation --circulation-only` | Контур и дальнейшие сценарии до 1060; конечный `launcher-passed.txt` |
| `--smoke-test output/smoke-reference --pk-reference-only` | Сводные ориентиры и старая ревизия; `pk-reference-passed.txt` |
| `--smoke-test output/smoke-curves --curve-fit-only` | Три подобранных протокола, пять кривых, replay/reset; `curve-fit-passed.txt` |

Например:

```sh
./target/debug/neuro-app --smoke-test output/smoke-curves --curve-fit-only
```

Успех — код завершения 0, нужный **конечный** маркер и осмотр соответствующих PNG. Один `passed.txt` полного прогона относится к раннему этапу, а не всем последующим модулям. Просматривайте легенды, шкалы, обрезание текста, выбор области, время, ошибки и отсутствие ложных значений.

После полного прогона можно проверить общий путь CLI:

```sh
./target/debug/neuro-cli run output/smoke-full/scenario.json --csv output/smoke-full/binding-cli.csv > /dev/null
cmp output/smoke-full/gui.csv output/smoke-full/binding-cli.csv
./target/debug/neuro-cli simulate output/smoke-full/brain-project.json --csv output/smoke-full/network-cli.csv > /dev/null
cmp output/smoke-full/network-gui.csv output/smoke-full/network-cli.csv
```

Имена артефактов задаёт `crates/neuro-app/src/smoke.rs`. Побайтное сравнение относится к одному toolchain/окружению; для иной платформы нужны объявленные численные допуски.

## Локальный macOS-пакет

```sh
./scripts/macos-app
codesign --verify --deep --strict output/NeuroLab.app
open output/NeuroLab.app
```

Скрипт использует `--locked --offline`, собирает **debug** executable, копирует атрибуции/метаданные, создаёт Info.plist и подписывает ad hoc. Это не release-оптимизация, Developer ID, notarization или установщик. При работающей старой копии сохраните проект и закройте её вручную перед обычным запуском новой: скрипт не мигрирует уже открытое окно.

Пакет проверяется отдельно теми же smoke-флагами через `output/NeuroLab.app/Contents/MacOS/NeuroLab`. Подпись меняет байты Mach-O, поэтому сырой SHA-256 подписанного файла может отличаться от `target/debug/neuro-app`; для идентичности сборки дополнительно используйте UUID и проверку поведения.

## Подготовка к GitHub

В репозитории должны оставаться код, Cargo.lock, исходные метаданные/ресурсы, примеры, документация и атрибуции. `.local/`, `target/`, `output/`, частные сцены и результаты исключены в `.gitignore`. Для обычной сборки Git LFS и скачивание полного атласа не нужны.

Перед коммитом:

1. Выполните проверки выше и исправьте относительные ссылки/примеры документации.
2. Проверьте `git diff --check`, `git status --short`, `git diff --stat`; убедитесь, что в diff нет секретов, частных данных, PDF/рисунков без условий распространения и результатов из output.
3. Для новых скриншотов проверьте соответствие текущему интерфейсу и сохраните атрибуцию BodyParts3D. Исходники научных рисунков не заменяйте скриншотами без происхождения.
4. Делайте сфокусированные коммиты после значимых проверенных изменений. Назовите изменение и приложите релевантные результаты в PR.

При явной авторизации отправки:

```sh
git fetch origin
git status --short --branch
git log --oneline origin/main..HEAD
git push origin main
```

Не используйте force push для обхода расхождения истории. Публичный GitHub Release — отдельное действие: для него нужны проверенная упаковка, сохранённые атрибуции и честный статус подписи/платформы. Этот репозиторий сам по себе не обещает готовый нотарифицированный бинарный релиз или настроенный GitHub Actions CI.

## Диагностика

| Симптом | Что проверить |
|---|---|
| `cargo-local` не найден/не запускается | Новый клон не содержит `.local/`; используйте установленный Cargo или отдельно подготовьте изолированный toolchain |
| `--offline` не находит пакет | Сначала `cargo fetch --locked` с сетью, не удаляя Cargo.lock |
| Ошибка линкера/SDK | Активные Command Line Tools/Xcode, архитектуру arm64 и совместимость установленного Rust |
| Первая сборка занимает время | Bevy/Metal собираются локально; не увеличивайте jobs выше 2 для обхода расхода памяти |
| JSON не открывается | Тип документа, `format_version`, модель/ревизию, атлас, неизвестные поля, конечность чисел, размер до 1 МиБ |
| Экспорт/сохранение блокируется | Пересчитать устаревшие результаты, включить требуемую гипотезу, разделить пути входа и выходов |
| Нет регионального CSV кофеина | В сценарии должна быть включена `perfusion`; полный контур имеет отдельный формат |
| Не видны точки исследования | Активный состав/дозы/одновременность не соответствуют источнику; загрузите целый исследовательский протокол |
| Нет движения в фоне | Воспроизведение ориентировано на активное окно; частицы также имеют отдельное визуальное время |
| Bindless отключён на Metal | Известный fallback bevy_egui; сверяйте работу и скриншот, не считайте предупреждение само по себе отказом |
| `block` / `__eh_frame` warning | Известные сообщения зависимостей/линкера; фактический статус в [verification.md](verification.md) |

При issue приложите commit SHA, ОС/архитектуру, `rustc --version`, команду, минимальный сценарий без частных данных, ожидаемый/фактический результат и релевантную часть stderr. Для научного расхождения добавьте первоисточник, единицы и способ сравнения.
