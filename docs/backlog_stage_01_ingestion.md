# Stage 1 Backlog: Data Ingestion (Rust + API)

## Scope
Получение блоков/транзакций/логов через JSON-RPC API.

## Tasks
- [x] Инициализировать Rust-проект.
- [x] Реализовать JSON-RPC клиент для `eth_getBlockByNumber`.
- [x] Добавить CLI-параметры: `--rpc-url`, `--start-block`, `--end-block`, `--full-tx`.
- [x] Добавить `eth_getLogs` и `eth_getTransactionReceipt`.
- [x] Добавить retry/backoff и timeout policy.
- [x] Добавить historical replay и near-real-time polling.

## Notes
- Текущий фокус: минимальный рабочий ingestion pipeline.
- Проверено на Arbitrum через Alchemy endpoint: `eth_getBlockByNumber` возвращает блоки корректно.
- Проверено `eth_getLogs` на диапазоне блоков `1..3` и `eth_getTransactionReceipt` на блоке `2`.
- Добавлена RPC policy: `--rpc-timeout-ms`, `--rpc-max-retries`, `--rpc-backoff-ms`, `--rpc-max-backoff-ms`.
- Добавлен polling-режим: `--follow` и `--poll-interval-ms`.
