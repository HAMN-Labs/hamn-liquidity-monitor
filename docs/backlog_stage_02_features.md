# Stage 2 Backlog: Feature Extraction

## Scope
Извлечение поведенческих признаков ликвидности.

## Tasks
- [x] Описать структуру фичей для swap / add-liquidity / remove-liquidity.
- [x] Реализовать парсер признаков из логов и receipt.
- [x] Добавить нормализацию признаков.
- [x] Написать unit-тесты на извлечение фичей.

## Notes
- Добавлен модуль `src/features/extractor.rs` с типами `LiquidityFeature`, `LiquidityEventKind`, `LiquidityAmounts`.
- Добавлен детектор событий по `topic0` (Swap/Mint/Burn) и парсер uint256-слов из `log.data`.
- Добавлена нормализация: `ln(1 + total_volume)`, относительный imbalance, `ln(1 + gas_used)`.
