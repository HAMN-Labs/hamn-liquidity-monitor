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

### In Progress
- [ ] Stage 1: добавить retry/backoff и timeout policy.
- [ ] Stage 1: historical replay и near-real-time polling.

### Rules
- Каждый выполненный пункт переносить в `Completed` с датой.
- Любые новые требования добавлять в backlog соответствующего этапа.
- Изменения по архитектуре фиксировать отдельной строкой в `Completed`.
