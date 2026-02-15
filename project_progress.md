# Project Progress

## 2026-02-15

### Completed
- [x] Создан каркас Rust-проекта (`cargo init`).
- [x] Создана папка `docs/` с backlog-файлами для Stage 1-6.
- [x] Зафиксирован процесс ведения прогресса в `project_progress.md`.
- [x] Реализован базовый ingestion блоков через JSON-RPC API (`eth_getBlockByNumber`).
- [x] Добавлены CLI-параметры запуска (`--rpc-url`, `--start-block`, `--end-block`, `--full-tx`).
- [x] Добавлен базовый unit-тест и проверена сборка (`cargo test`).
- [x] Добавлен `docs/workflow.md` с правилами ведения backlog и прогресса.
- [x] Проверен запуск ingestion на Arbitrum через Alchemy API (`eth_getBlockByNumber`, блок `1`).
- [x] Добавлены RPC-методы `eth_getLogs` и `eth_getTransactionReceipt` в ingestion-клиент.
- [x] Добавлены CLI-опции `--fetch-logs` и `--receipt-limit` для валидации данных.
- [x] Проверены на Arbitrum: логи (`1..3`) и receipt (блок `2`, `status=1`).
- [x] Добавлена retry/backoff/timeout policy для RPC-запросов.
- [x] Добавлен polling-режим (`--follow`) для near-real-time обработки блоков.
- [x] Проверены replay и polling-сценарии на Arbitrum через Alchemy endpoint.
- [x] Реализован Stage 2 feature extraction модуль (`swap/add/remove liquidity`).
- [x] Добавлен парсинг признаков из `receipt.logs` и нормализация числовых фичей.
- [x] Добавлен runtime-флаг `--extract-features` для извлечения фичей из полученных receipt.
- [x] Добавлены unit-тесты Stage 2 (в сумме 6 тестов по проекту, все проходят).
- [x] Реализован Stage 3 adaptive memory модуль (`src/memory/adaptive.rs`).
- [x] Добавлены `match/new pattern` + confidence/recency обновления при `observe`.
- [x] Добавлена snapshot сериализация памяти (`--memory-snapshot-in`, `--memory-snapshot-out`).
- [x] Добавлена интеграция памяти в runtime через `--enable-memory`.
- [x] Добавлены unit-тесты Stage 3, общий статус тестов: 9/9 passed.
- [x] Реализован Stage 4 stabilization: decay confidence + pruning stale/noisy/weak patterns.
- [x] Добавлены лимиты памяти (`memory_max_patterns`) и retention ranking при переполнении.
- [x] Добавлены runtime-метрики памяти: `active_patterns`, `match_ratio`, `churn_rate`.
- [x] Добавлены тесты Stage 4, общий статус тестов: 12/12 passed.
- [x] Реализован Stage 5 sequence mapping модуль (`src/sequences/transition.rs`).
- [x] Добавлено хранение последовательностей действий по сущностям (pool-based).
- [x] Построен transition map с online-обновлением вероятностей.
- [x] Добавлено Laplace smoothing для редких переходов.
- [x] Добавлены тесты Stage 5, общий статус тестов: 15/15 passed.
- [x] Реализован Stage 6 online runtime pipeline (producer/consumer на bounded `mpsc`).
- [x] Добавлен backpressure через `--pipeline-queue-capacity`.
- [x] Добавлены runtime-метрики latency/throughput/errors/lag.
- [x] Проведена e2e валидация на Arbitrum (Alchemy, блоки `2..20`).
- [x] Добавлен детализированный план следующей итерации (`docs/NEXT_ITERATION_PLAN.md`).
- [x] Добавлена пользовательская документация на английском (`docs/USER_GUIDE.md`).
- [x] Обновлен `README.md` ссылками на пользовательскую документацию и план.
- [x] Добавлен CLI-справочник на английском (`docs/CLI_REFERENCE.md`).
- [x] Добавлен operations runbook на английском (`docs/OPERATIONS_RUNBOOK.md`).
- [x] Обновлены ссылки в `README.md` и `docs/USER_GUIDE.md` на новые документы.
- [x] Добавлен набор готовых сценариев запуска (`docs/EXAMPLES.md`).
- [x] Обновлены ссылки в `README.md` и `docs/USER_GUIDE.md` на examples.
- [x] Добавлен FAQ на английском (`docs/FAQ.md`).
- [x] Обновлены ссылки в `README.md` и `docs/USER_GUIDE.md` на FAQ.
- [x] Стартована следующая фаза: добавлены конфигурируемые log-фильтры (`--log-topic0`, `--log-address`) для `eth_getLogs`.
- [x] Обновлена англоязычная документация по фильтрам (`README.md`, `docs/USER_GUIDE.md`, `docs/CLI_REFERENCE.md`, `docs/EXAMPLES.md`, `docs/FAQ.md`).

### In Progress
- [ ] Подготовить следующий итерационный шаг: улучшение качества детекции на блоках с реальными DeFi-логами.

### Rules
- Каждый выполненный пункт переносить в `Completed` с датой.
- Любые новые требования добавлять в backlog соответствующего этапа.
- Изменения по архитектуре фиксировать отдельной строкой в `Completed`.
