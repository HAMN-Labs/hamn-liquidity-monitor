# Stage 5 Backlog: Sequence Mapping

## Scope
Построение переходов и вероятностей.

## Tasks
- [x] Реализовать хранение последовательностей действий.
- [x] Построить transition map.
- [x] Добавить online-обновление вероятностей.
- [x] Добавить сглаживание для редких переходов.

## Notes
- Добавлен модуль `src/sequences/transition.rs`.
- Хранение последовательностей реализовано по сущности `pool_address` (`last_event_by_entity`).
- Transition map: `transition_counts[(from, to)]` + `outgoing_counts[from]`.
- Вероятности:
  - raw: `P(to | from) = count(from,to) / count(from,*)`,
  - smoothed (Laplace): `(count + alpha) / (outgoing + alpha * K)`, где `K=3`.
