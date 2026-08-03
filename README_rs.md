# MetalPython (Rust rewrite)

This tree is a **Rust rewrite of MicroPython** aimed at Metal OS and related hosts.
Upstream MicroPython lives alongside it as a **read-only reference**.

For the original project overview, ports, and design values, see [`README.md`](README.md).
This file documents only the rewrite layer.

## Upstream vs rewrite

| Upstream (do **not** edit) | Rust shadow (work here) |
|----------------------------|-------------------------|
| `py/` | `py_rs/` |
| `shared/` | `shared_rs/` |
| `extmod/` | `extmod_rs/` |
| `ports/unix`, `ports/qemu`, … | `ports_rs/unix`, `ports_rs/qemu`, … |
| `mpy-cross/` | `mpy-cross_rs/` |
| `lib/`, `tools/`, C under `ports/` | leave alone |

Also edit freely:

- Root [`Cargo.toml`](Cargo.toml) / `Cargo.lock`
- [`rewrite/`](rewrite/) — scaffolding, symmetry, generators
- [`.github/workflows/metalpython_rs.yml`](.github/workflows/metalpython_rs.yml)

**Hard rule:** never modify upstream C/Python under `py/`, `extmod/`, `shared/`, `ports/`, `lib/`, `tools/`, or `mpy-cross/`. Compare against those trees; implement depth in `*_rs`.

`sys.implementation.name` is `metalpython` (upstream is `micropython`).

## Layout

```
Cargo.toml          workspace (py_rs, shared_rs, extmod_rs, ports_rs/*, mpy-cross_rs)
py_rs/              core VM, compiler, builtins (↔ py/)
shared_rs/          shared runtime helpers (↔ shared/)
extmod_rs/          extension modules (↔ extmod/)
ports_rs/unix/      host binary `metalpython` (↔ ports/unix)
ports_rs/qemu/      qemu-oriented port stub
mpy-cross_rs/       cross-compiler (↔ mpy-cross)
rewrite/            symmetry reports, generators, scaffolding
```

File + `pm_mpy_*` API symmetry is tracked at **100%** (one shadow stem per upstream unit). Remaining work is **behavioral depth** vs C MicroPython, not missing files.

## Build & run

Requires a recent stable Rust toolchain.

```bash
# Optional: keep cargo artifacts out of the tree
export CARGO_TARGET_DIR=/tmp/metalpython-cargo-target

cargo build -p ports_rs_unix
cargo run -q -p ports_rs_unix          # interactive REPL
cargo run -q -p ports_rs_unix -- -c 'print(40+2)'
```

Binary name: `metalpython` (package `ports_rs_unix`).

Cross-compiler:

```bash
cargo run -q -p mpy_cross_rs -- -o /tmp/foo.mpy /tmp/foo.py
```

## Test & lint

```bash
cargo check --workspace
cargo test -p py_rs --lib -- --test-threads=1    # GC is not parallel-safe in tests
cargo test -p extmod_rs --lib -- --test-threads=1
cargo clippy --workspace --all-targets -- -D warnings
python3 -m rewrite.symmetry --fail-on-regression
```

CI for the rewrite is [`.github/workflows/metalpython_rs.yml`](.github/workflows/metalpython_rs.yml) (check, tests, smoke, mpy-cross roundtrip, symmetry).

## Symmetry

```bash
python3 -m rewrite.symmetry
python3 -m rewrite.symmetry --fail-on-regression
```

Reports file coverage (`py/` → `py_rs/`, …) and the `pm_mpy_*` module surface discovered from upstream `MP_REGISTER_MODULE*`.

## Comparing to C MicroPython

Build upstream unix as usual (`ports/unix`, variant `standard`), then probe side by side:

```bash
./ports/unix/build-standard/micropython -c '…'
cargo run -q -p ports_rs_unix -- -c '…'
```

Ignore noise such as object hashes / addresses, `sys.implementation.name`, and non-deterministic float streams unless you are chasing PRNG parity.

## Metal OS notes

Host networking for Metal guests uses Metal/lwIP faces under the Metal OS tree — do not invent WiFi/BLE/cyw43 stacks inside MetalPython. Unix POSIX socket path is for the host port.

## License

Same MIT lineage as MicroPython; see workspace `license` and upstream licensing notices.
