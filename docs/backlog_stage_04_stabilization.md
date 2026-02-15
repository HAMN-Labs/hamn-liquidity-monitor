# Stage 4 Backlog: Memory Stabilization

## Scope
Стабилизация и очистка памяти.

## Tasks
- [x] Реализовать decay веса паттернов.
- [x] Добавить pruning устаревших и шумовых паттернов.
- [x] Ввести лимиты на размер памяти.
- [x] Добавить метрики стабильности (churn, active patterns, match ratio).

## Notes
- Добавлен `StabilizationConfig` в `src/memory/adaptive.rs`.
- Реализован `stabilize(current_block)`:
  - decay confidence по inactivity,
  - pruning по weak/stale/noisy критериям,
  - ограничение `max_patterns` через retention score.
- Добавлены runtime-метрики памяти:
  - `active_patterns`,
  - `match_ratio`,
  - `churn_rate`,
  - счетчики observed/matched/created/pruned.
