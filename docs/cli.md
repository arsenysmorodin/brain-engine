# Справочник CLI

[Индекс](README.md) · [Форматы](data-formats.md)

## Запуск и вывод

Из корня репозитория:

```sh
cargo build -p neuro-cli --locked
./target/debug/neuro-cli --help
```

Можно использовать `cargo run -p neuro-cli --locked -- <команда>`. В изолированной рабочей копии замените `cargo` на `./scripts/cargo-local`; после кеширования зависимостей допускается `--offline` перед `--`.

Пути относительны текущему каталогу. Подкоманд `--help` у отдельных команд нет: используйте общий `--help`. Без аргументов печатается справка. Успех — код 0; ошибки валидации, файлов и аргументов — stderr и код 1. `new*` печатает сообщение о создании; расчёты, `atlas`, подбор и benchmarks выводят JSON в stdout. Большие трассы лучше перенаправлять в файл или `/dev/null`.

## Команды

| Команда | Назначение | Дополнительные аргументы |
|---|---|---|
| `atlas` | Проверить встроенные атлас/геометрию, вывести анатомический инвентарь | `--csv PATH` |
| `new PATH` | Создать абстрактный A/B-сценарий | нет |
| `new-response PATH` | Создать Black–Leff-сценарий | нет |
| `new-network PATH` | Создать демонстрационную E/I-сеть | нет |
| `new-caffeine PATH` | Создать отдельный кофеиновый сценарий | нет |
| `new-circulation PATH` | Создать новый общий тройной сценарий, ревизия 4 | нет |
| `run PATH` | A/B или Black–Leff по `model_id` | `--csv PATH`, `--json PATH` |
| `simulate PATH` | Сетевой сценарий или `brain_project` | `--csv PATH`, `--json PATH` |
| `caffeine PATH` | Кофеиновый сценарий или проект | `--csv PATH`, `--json PATH`, `--regional-csv PATH`, `--network-at-min N` |
| `circulation PATH` | Сценарий полного контура или проект | те же четыре флага |
| `fit-purine-curves PATH` | Подбор набора оцифрованных плазменных кривых | `--json PATH`, `--csv PATH`, `--scenarios-dir DIR` |
| `benchmark-response` | Проверка теоретического Black–Leff-эталона | нет |
| `benchmark-purine-pk` | Сверка сводных ориентиров ревизий 2/3 | нет |

Повторный/неизвестный флаг отклоняется. `new*` требует нового имени файла. Для расчётов входной файл защищён от перезаписи выходом, а разные виды экспорта должны иметь разные пути. Уже существующий **выходной** файл может быть заменён — сохраняйте нужные результаты под отдельными именами.

## Основные примеры

```sh
mkdir -p output/cli-demo
./target/debug/neuro-cli atlas --csv output/cli-demo/atlas.csv > output/cli-demo/atlas.json
./target/debug/neuro-cli run scenarios/competition.json --csv output/cli-demo/binding.csv > output/cli-demo/binding.json
./target/debug/neuro-cli run scenarios/response.json --csv output/cli-demo/response.csv > output/cli-demo/response.json
./target/debug/neuro-cli simulate scenarios/brain-network.json --csv output/cli-demo/network.csv > output/cli-demo/network.json
```

`atlas` экспортирует только анатомию с `activity_status=not_configured`. Он не загружает сеть и не заменяет `simulate`.

Отдельная модель кофеина с перфузией:

```sh
./target/debug/neuro-cli caffeine scenarios/caffeine-perfusion.json \
  --csv output/cli-demo/caffeine.csv \
  --regional-csv output/cli-demo/caffeine-regions.csv \
  --json output/cli-demo/caffeine-report.json > /dev/null
```

`--regional-csv` для кофеина требует включённую в сценарии `perfusion`. Локальные массы этой модели не складываются с общей системной массой.

Новый полный контур:

```sh
./target/debug/neuro-cli new-circulation output/cli-demo/purines.json
./target/debug/neuro-cli circulation output/cli-demo/purines.json \
  --csv output/cli-demo/purines.csv \
  --regional-csv output/cli-demo/purines-regions.csv > output/cli-demo/purines-report.json
```

Файл `scenarios/purine-circulation.json` — историческая ревизия 1. Его повторный запуск сохраняет ту ревизию; `new-circulation` — текущий новый референс, не миграция файла.

## Сравнение E/I по PK-снимку

Сохраните из GUI проект с явно включённой и настроенной нейронной гипотезой:

```sh
./target/debug/neuro-cli circulation neuro-purine-project.json \
  --network-at-min 60 --json output/cli-demo/neural-report.json > /dev/null
```

То же возможно для `caffeine`. Флаг не включает гипотезу автоматически: она должна присутствовать и быть включена во входе. `N` задаётся в минутах, внутри горизонта PK. Возвращаются две сетевые трассы с миллисекундным временем, а не непрерывная совместная симуляция всех временных масштабов.

При `circulation` без сравнения JSON содержит `CirculationTrace`; с флагом — объект `trace` / `network_comparison`. У `caffeine` JSON всегда содержит `trace` и `network_comparison` (при отсутствии сравнения — null). CSV остаётся PK-трассой; JSON дополнительно содержит сетевые результаты.

## Подбор опубликованных кривых

```sh
./target/debug/neuro-cli fit-purine-curves data/purine-pk-curves.json \
  --json output/cli-demo/fit.json \
  --csv output/cli-demo/fit.csv \
  --scenarios-dir output/cli-demo/fitted-protocols > /dev/null
```

`--scenarios-dir` должен указывать **новый отдельный каталог**, не содержащий пути JSON/CSV самого отчёта. Для встроенного набора будут созданы:

| Файл | Протокол |
|---|---|
| `protocol-1.json` | Mondal: M100 |
| `protocol-2.json` | Mondal: C150 |
| `protocol-3.json` | Wang: C150+M100+T50 |
| `comparison.json` | Ошибки фактического полного контура по каждой сопоставимой кривой |

Порядок относится к встроенному набору; для другого набора читайте состав в каждом JSON. Неполный состав, для которого нет кривой каждого присутствующего вещества, не получает выдуманный полный протокол.

```sh
./target/debug/neuro-cli circulation output/cli-demo/fitted-protocols/protocol-3.json \
  --csv output/cli-demo/fitted-triple.csv > output/cli-demo/fitted-triple-report.json
```

Подбор детерминированный, ограниченный и многозапусковый; глобальный оптимум не гарантируется. Проверка исключаемых точек не является независимой валидацией. [Алгоритм, единицы и ограничения](full-pk-calibration.md).

## Benchmarks

```sh
./target/debug/neuro-cli benchmark-response > output/cli-demo/response-benchmark.json
./target/debug/neuro-cli benchmark-purine-pk > output/cli-demo/pk-benchmark.json
```

Первый проверяет теоретический эталон без шума и возвращает ошибку при превышении допуска. Второй воспроизводит **исторические ревизии 2/3**, не проверяет текущие подобранные кривые ревизии 4 и не объявляет физиологическую валидацию.
