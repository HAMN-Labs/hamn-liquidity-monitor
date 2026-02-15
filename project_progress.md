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

### In Progress
- [ ] Stage 4: реализовать decay веса паттернов.

### Rules
- Каждый выполненный пункт переносить в `Completed` с датой.
- Любые новые требования добавлять в backlog соответствующего этапа.
- Изменения по архитектуре фиксировать отдельной строкой в `Completed`.
