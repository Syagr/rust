# Practical Work 5 — Потоки, канали та шифрування

This crate implements the tasks for Практична робота 5.

Usage examples:

Build:
```bash
cargo build --release
```

Matrix test (smaller size for local test):
```bash
cargo run -- matrix --size 256 --count 2
```

Encrypt directory (generate a key automatically):
```bash
cargo run -- encrypt --dir ./some_folder
```

Encrypt directory with your base64 key:
```bash
cargo run -- encrypt --dir ./some_folder --key <BASE64_32BYTE_KEY>
```

Notes:
- `matrix` subcommand: producer creates `count` matrices of size `size x size` and broadcasts them to two consumer threads. Sums are computed using `rayon`.
- `encrypt` subcommand: a producer walks the directory, reads files and sends (path, bytes) pairs to a channel consumed by 3 worker threads. Each worker encrypts with AES-256-GCM and writes `<original_filename>.data`. A monitor thread prints the processed-file counter when it changes.
