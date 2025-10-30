
# AppFlowy-Collab

AppFlowy-Collab is the collaborative data layer that powers AppFlowy clients. It exposes `collab`, a Rust crate built on
top of [yrs](https://docs.rs/yrs/latest/yrs/), packaging CRDT primitives, persistence helpers, and domain objects for
documents, databases, folders, importers, plugins, and user state. The crate can be embedded inside desktop, mobile, or
cloud services that need to read and apply AppFlowy collaboration updates.

## Highlights
- Unified crate that orchestrates the AppFlowy collaboration stack: documents, databases, folders, importer tooling,
  plugins, and user awareness.
- Strongly typed API layered on yrs, with transaction helpers and schema mappers that keep CRDT updates ergonomic.
- Pluggable storage and sync via the `plugins` feature (RocksDB for local caches, Supabase sync, plus custom
  extensions).
- Comprehensive importer and converter support: Markdown/Notion import pipelines, CSV import (optional), and
  markdown/plain text exports.
- Integration tests that cover document editing, database history, importer pipelines, and plugin behaviour.

## Architecture Overview
The AppFlowy runtime connects three domain services—`flowy-folder`, `flowy-database`, and `flowy-document`—through the
`Collab` object supplied by this crate. Core responsibility highlights:

1. Client code requests domain operations (e.g. create a view, update a document block) via the domain modules in
   `collab`.
2. `Collab` records the change as a yrs transaction and fires plugin hooks.
3. Plugins persist the change (e.g. `RocksdbDiskPlugin`) and optionally sync it to remote services (`SupabaseDBPlugin`,
   custom websocket backends).
4. `Collab` fans the update out to any other connected clients, keeping local caches and UI layers in sync.

The sequence diagrams in `docs/architecture.md` walk through common flows—creating, opening, editing, and synchronising
documents—and mirror the behaviour of other collab objects such as databases and user awareness.

## Module Map
The `collab` crate organises its API into cohesive modules:

- `collab::core` – yrs wrappers, transactions, `Collab` abstractions, plugin registry, snapshot utilities.
- `collab::document` – block tree, text manipulation, markdown/plain text conversion, rich-text schema helpers.
- `collab::database` – view schemas, row/value models, history, relation remappers, CSV helpers (with `import_csv`
  feature).
- `collab::folder` – workspace hierarchy, migrations, folder/view metadata.
- `collab::importer` – Notion/Markdown import pipelines, async zip tooling, filesystem utilities.
- `collab::plugins` – trait definitions plus reference implementations (RocksDB disk cache, Supabase sync bridge).
- `collab::user` – user awareness state (appearance, reminders) and collaboration helpers.
- `collab::entity` – protobuf-generated entity definitions shared across modules.
- `collab::tests` (integration) – scenarios that exercise end-to-end flows across the modules.

## Cargo Features
- `plugins` (default: off) – enables plugin traits and the bundled RocksDB-backed implementations. Required for
  integration tests under `tests/plugins`.
- `verbose_log` – emits additional tracing when debugging field-level changes.
- `trace_transact` – instrument yrs transactions for profiling and troubleshooting.
- `lock_timeout` / `rwlock_reason` – advanced locking diagnostics for long-running async hosts.
- `import_csv` – unlocks CSV ingestion support in the database importer.

Enable features when building or testing via `cargo build --features plugins`.

## Getting Started
### Prerequisites
- Rust `1.85` toolchain (`rustup toolchain install 1.85` if needed).
- `cargo` and `rustfmt` from the same toolchain.
- Optional: [`cargo-make`](https://sagiegurari.github.io/cargo-make/) for the coverage task, `grcov` +
  `llvm-tools-preview` for LCOV export.

### Build & Test
```bash
# Compile the crate
cargo build

# Run the full test suite
cargo test

# Exercise plugin integration tests
cargo test --features plugins --test plugins
```

### Coverage (optional)
Use the bundled Cargo Make tasks to produce an LCOV report:

```bash
cargo make check_grcov          # once, validates tooling
cargo make run_coverage         # runs instrumented tests
```

The coverage report is written to `target/coverage.lcov`.

## Repository Layout
```
collab/            # Collab crate source
  src/
    core/          # yrs core wrappers and Collab orchestration
    document/      # document model and text tooling
    database/      # database views, rows, and history
    folder/        # workspace/folder hierarchy
    importer/      # Notion/Markdown import logic
    plugins/       # plugin API and reference implementations
    user/          # user awareness collab objects
  tests/           # integration tests (cargo test)
docs/              # Architecture notes and UML diagrams
resources/         # Architecture images referenced by the README
```

## Further Reading
- `docs/architecture.md` – deep dive diagrams covering create/open/edit/sync flows and custom collab objects.
- `collab/tests/` – integration scenarios demonstrating API usage.
- [`yrs` documentation](https://docs.rs/yrs/latest/yrs/) – underlying CRDT primitives used throughout `collab`.

Contributions are welcome—start by exploring the architecture docs, running the tests, and opening a discussion in the
AppFlowy community if you plan to introduce new collab objects or plugins.
