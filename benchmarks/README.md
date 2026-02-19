# Benchmarks

Benchmark reports are written here by `shard-load-test`:

- `benchmark-<timestamp>.json`
- `benchmark-<timestamp>.md`

Example:

```bash
cd desktop/rust
cargo run --release --bin shard-load-test -- --base-url http://127.0.0.1:9091 --requests 1000 --concurrency 1000 --mode all --out-dir ../../benchmarks
```
