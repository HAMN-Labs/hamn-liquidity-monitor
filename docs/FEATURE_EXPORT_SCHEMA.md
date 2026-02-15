# Feature Export Schema (NDJSON)

Each line in `--features-out` is a JSON object:

```json
{
  "block": 359066952,
  "event": "swap",
  "tx": "0x...",
  "pool": "0x...",
  "volume_ln": 2.918365,
  "imbalance": 0.017436,
  "gas_ln": 11.102901
}
```

## Fields
- `block` (`u64`): block number where feature was produced.
- `event` (`string`): one of `swap`, `add_liquidity`, `remove_liquidity`.
- `tx` (`string`): transaction hash.
- `pool` (`string`): pool address associated with the parsed log.
- `volume_ln` (`f64`): normalized log-volume proxy.
- `imbalance` (`f64`): normalized token flow imbalance.
- `gas_ln` (`f64`): normalized gas proxy.

## Rotation
If `--features-out-rotate-records N` is set (`N > 0`), files are split by record count:
- `features.part000000.ndjson`
- `features.part000001.ndjson`
- ...
