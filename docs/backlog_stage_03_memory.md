# Stage 3 Backlog: Adaptive Memory

## Scope
Матчинг векторов и хранение паттернов.

## Tasks
- [x] Определить модель паттерна и метаданные confidence/recency.
- [x] Реализовать логику match/new pattern.
- [x] Добавить snapshot сериализацию.
- [x] Добавить базовые тесты matching logic.

## Notes
- Добавлен модуль `src/memory/adaptive.rs` с `AdaptiveMemory`, `Pattern`, `MemorySnapshot`.
- Вектор паттерна: `[volume_ln, imbalance, gas_ln]`, с `event_kind`-сегментацией.
- Реализована логика `observe`: nearest-neighbor по евклидову расстоянию + порог `distance_threshold`.
- Метаданные паттерна: `occurrences`, `confidence`, `last_seen_block`.
- Добавлены snapshot методы: `save_snapshot` / `load_snapshot` (JSON).
