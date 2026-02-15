# Stage 6 Backlog: Online Runtime

## Scope
Полноценная потоковая обработка в production-like режиме.

## Tasks
- [x] Собрать ingestion -> features -> memory -> sequences pipeline.
- [x] Добавить backpressure и bounded очереди.
- [x] Добавить метрики latency/throughput/errors/lag.
- [x] Провести e2e валидацию на выбранной сети.

## Notes
- Реализован bounded pipeline на `tokio::sync::mpsc`:
  - producer: ingestion (blocks/receipts/logs),
  - consumer: features -> memory -> sequences.
- Backpressure реализован через ограниченную очередь `--pipeline-queue-capacity`.
- Добавлены runtime-метрики:
  - `throughput_rps`,
  - `avg_block_fetch_ms`,
  - `avg_receipt_fetch_ms`,
  - `avg_queue_backpressure_ms`,
  - `avg_queue_latency_ms`,
  - `avg_processing_ms`,
  - `rpc_errors`,
  - `max_block_lag`.
- E2E прогон выполнен на Arbitrum (Alchemy endpoint) в диапазоне блоков `2..20`.
