# Проверка единого запуска и представлений веществ

25.10.2026, macOS Apple Silicon, Rust 1.99.1 через `scripts/cargo-local`. Проверка численного и интерфейсного поведения; физиологическая калибровка из неё не следует.

## Автоматические проверки

76 тестов: 31 engine, 11 domain, 9 CLI, 25 app. Пройдены `fmt --all --check`, `test --workspace --locked`, `clippy --workspace --all-targets --locked -- -D warnings`, `build --workspace --locked`. Cargo.lock сохранён. `scripts/macos-app` пересобирает пакет, проверены Info.plist и ad hoc codesign.

Новая гипотеза `purine_illustrative_drive_v1`, ревизия 1, проверяется на аналитическом значении g/2 при Ct=S, знаке коэффициентов, нулевом входе, региональности и совпадении с исходной сетью при отсутствии входов. Выключенная гипотеза и время вне трассы отклоняются. GUI и CLI используют один вычислительный путь. Ревизия 2 циркуляции допускает отложенный общий момент приёма для ограниченного профиля взаимодействий; тест проверяет перенос во времени и сохранение прежней семантики ревизии 1. Разнесённые/повторные приёмы не получают неподдерживаемую поправку взаимодействий.

Указательные тесты egui нажимают кнопку запуска, узлы и стрелку общей карты. Проверяются запятая в числовом вводе, недопустимые дозы/время, запуск теакрина и сохранение прежнего результата при ошибке. Изменение настроек гипотезы не меняет PK, а изменение времени/доз сбрасывает E/I-сравнение. Невалидная гипотеза не оставляет доступным старый экспорт. Проект сохраняет гипотезу и сеть; отдельный CLI-тест воспроизводит теакрин с `--network-at-min`.

## Нативное приложение и визуальная проверка

Полный прогон пакета:

```sh
output/NeuroLab.app/Contents/MacOS/NeuroLab --smoke-test output/unified-launcher-release
```

Пройдены кадры 0…1060, включая прежние анатомические, рецепторные, сетевые, кофеиновые, перфузионные и учебные сценарии. Финальный `launcher-passed.txt` записан перед штатным `AppExit::Success`. Это настоящий процесс Bevy/Metal и снимки его окна; состояния задаёт harness, а указательные тесты используют синтетические события egui. Ручная проверка новых элементов средствами macOS не заявляется. Старый экземпляр пользователя не закрывался.

Новый сценарий проходит метиллиберин 100 мг на 30-й минуте, теакрин 50 мг на 10-й минуте и тройной протокол с общим временем 5 мин. Проверены форма, ошибочный черновик и отмена, воспроизведение, 3D, региональные графики, карты пути, баланс массы, отдельное E/I-сравнение и кофеиновая PET-кривая. Сохранение → изменение → открытие возвращает протокол и идентичный PK-CSV; экспорт поверх исходного файла и устаревшего результата блокируется.

Изображения в `output/unified-launcher-layout` осмотрены: крупный мозг при скрытом графике, читаемые показатели/баланс при включённом, карта с раздельными стрелками PK-взаимодействий и прямого учебного входа Ct → E/I. В полном прогоне пакета повторно сохранены соответствующие снимки в `output/unified-launcher-release`: `launcher-methylliberine.png`, `launcher-invalid.png`, `methylliberine-default-3d.png`, `methylliberine-regional-pk.png`, `methylliberine-pathway.png`, `methylliberine-neural-hypothesis.png`, `theacrine-mass.png`, `theacrine-pathway.png`, `theacrine-neural-hypothesis.png`, `launcher-triple.png`, `triple-unified-pk.png`, `caffeine-pet-unified.png`, `launcher-learning-scale.png`.

По умолчанию график скрыт. Финальная подпись серого слоя ΔE уточнена: «нет E/I-данных» относится и к отсутствующему сравнению, и к структурам без популяций. Учебник содержит 153 общих термина и 143 анатомические записи; новые g, S, ΔP_E и пероральный приём объясняются отдельно.

После уточнения подписи пакет пересобран и проверен повторным прогоном `--smoke-test output/unified-launcher-final --launcher-only` (кадры 900…1060). Получен `launcher-passed.txt`, процесс завершился с кодом 0; codesign прошёл повторно. Это завершающий прогон именно упакованной версии.

## Совпадение GUI и CLI

```sh
target/debug/neuro-cli circulation output/unified-launcher-release/methylliberine-neural-project.json --network-at-min 35 --csv output/unified-launcher-release/methylliberine-pk-cli.csv --json output/unified-launcher-release/methylliberine-neural-cli.json > /tmp/neurolab-unified-neural-cli-release.json
target/debug/neuro-cli circulation output/unified-launcher-release/launcher-triple-project.json --csv output/unified-launcher-release/launcher-triple-cli.csv --regional-csv output/unified-launcher-release/launcher-triple-regions-cli.csv > /tmp/neurolab-unified-triple-cli-release.json
```

PK-CSV метиллиберина, общая и региональная CSV тройного протокола совпали побайтно с GUI. Полное `network_comparison` из CLI совпало с `methylliberine-neural-gui.json` по структуре и всем значениям. Артефакты `output/` игнорируются Git.

## Границы результата

Ввод — пероральный, время — минуты от начала сценария. Ограничения формы технические; это не выбор безопасной дозы. Измеренная человеческая рецепторная калибровка M/T отсутствует: кофеиносодержащая PET-кривая им не присваивается. Количественный переход Ct → E/I — выключенная по умолчанию учебная гипотеза с явными g/S, а не установленный механизм или клинический эффект. Сетевой опыт использует замороженный PK-снимок и собственную миллисекундную шкалу; непрерывная совместная PK/нейронная динамика не реализована. Ограничения источников и формулы находятся в [circulation-model.md](circulation-model.md).

Сохраняются известные предупреждения linker `__eh_frame`, транзитивной `block 0.1.6` и fallback Metal без bindless textures. Все размеры окон/экраны/DPI, VoiceOver и длительная ручная сессия не проверялись. Финальная сборка подписана ad hoc; notarization не выполнялась.
