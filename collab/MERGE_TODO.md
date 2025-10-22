# Merge TODO: Consolidate Satellite Crates into `collab`

This document tracks the work required to merge the standalone crates
(`collab-entity`, `collab-plugins`, `collab-document`, `collab-folder`,
`collab-user`, `collab-database`, and `collab-importer`) into the primary
`collab` crate. Tasks are ordered from lowest-level dependencies to the
highest-level consumers so that each merge simplifies the remaining ones.

> **Move-only policy:** Each merge should relocate code and adjust module
> imports without altering business logic or behavior. Fix compiler errors by
> updating `mod` declarations and `use` paths rather than rewriting
> implementations.

## Overall Preparation

- [ ] Agree on success criteria (public API surface, feature flags, WASM
      support, build targets).
- [ ] Capture a dependency snapshot (workspace `Cargo.toml`, crate feature
      matrices, `build.rs` scripts, protobuf generation, and CI steps).
- [ ] Set up a tracking branch and ensure the workspace builds and tests pass
      prior to any changes.
- [ ] Document the current module layout of `collab` to decide where each
      crate's code will live after consolidation.
- [ ] Establish a move-only checklist: confirm every commit limits itself to
      file moves, module path updates, and import fixes (no functional edits).
- [ ] Capture a map of current `use` paths so import updates can be applied
      immediately after each relocation.

## Merge Order and Tasks

### 1. Integrate `collab-entity` (foundation crate)

- [x] Move `collab-entity/src` modules into a new `collab::entity` (or similar)
      module tree, preserving protobuf-generated code structure.
- [x] Fold `collab-entity`'s `build.rs` (prost invocation) and `proto/`
      artifacts into the `collab` build script, ensuring path updates and reuse
      of generated files.
- [x] Copy crate feature flags (if any) and shared dependencies (`prost`,
      `bytes`, etc.) into `collab/Cargo.toml`, deduplicating versions.
- [x] Replace `collab-entity` imports across the workspace with the new
      in-crate module path; remove the dependency from workspace members.
- [x] Run `cargo fmt`, `cargo check --all-features`, and targeted tests for
      crates previously depending on `collab-entity`.

### 2. Integrate `collab-plugins` (optional extensions)

- [x] Relocate `collab-plugins/src` into `collab::plugins`, preserving
      conditional `rocksdb`/WASM support.
- [x] Merge any plugin-specific feature flags and dependencies (e.g.,
      `tokio-retry`, optional `rand`, `rocksdb`) into `collab/Cargo.toml`.
- [x] Update `build.rs` or initialization hooks if plugins require runtime
      registration; document the new public API surface.
- [x] Adjust downstream crates' dev-dependencies to use the in-crate modules.

### 3. Integrate `collab-document` (document model)

- [x] Move document modules into `collab::document`, ensuring the async APIs
      still expose the same types.
- [x] Consolidate overlapping dependencies (`markdown`, `nanoid`,
      `tokio-stream`) and align feature gating for WASM builds.
- [x] Update crate exports and re-export any types that other crates expect
      from `collab-document`.
- [x] Remove the crate from the workspace manifest and update import paths.

### 4. Integrate `collab-folder` (folder hierarchy)

- [x] Migrate folder modules into `collab::folder`, including observers,
      migration utilities, and section management code.
- [x] Resolve naming conflicts with existing `collab` modules (especially
      shared types such as `ViewId`).
- [x] Transfer tests and fixtures; adapt them to the consolidated module
      layout.
- [x] Drop the workspace dependency and update consumers (e.g., importer,
      database) to the new module path.

### 5. Integrate `collab-user` (user/session services)

- [x] Embed user management modules under `collab::user` and relocate async
      task handling utilities.
- [x] Merge dependency declarations (`tokio`, `tokio-stream`) and ensure
      feature flags or cfg gates remain accurate for WASM builds.
- [x] Update references in higher-level crates and remove the standalone
      package.

### 6. Integrate `collab-database` (structured data layer)

- [x] Bring database modules into `collab::database`, including import/export
      helpers, locking abstractions, and type definitions.
- [x] Reconcile overlapping dependencies (e.g., `rayon`, `csv`, `rust_decimal`,
      timezone utilities) with the consolidated `collab` manifest.
- [x] Port optional features such as `verbose_log` and `import_csv`, ensuring
      they remain toggleable after the merge.
- [x] Update all runtime and dev consumers (including tests) to the new module
      path, then remove the crate entry from the workspace.

### 7. Integrate `collab-importer` (top-level orchestration)

- [x] Move importer modules into `collab::importer`, wiring them to the newly
      in-house folder, database, and document modules.
- [x] Simplify dependency configuration now that the importer references
      in-crate modules; remove duplicate third-party dependencies where
      possible.
- [x] Ensure importer-specific assets (e.g., `sanitize-filename`, `async_zip`)
      are justified in the merged crate and documented.
- [x] Run the importer integration tests and regression scenarios that combine
      folder/document/database flows.

## Final Cleanup

- [x] Delete the obsolete crate directories and adjust the workspace
      `Cargo.toml`/`Cargo.lock` entries.
- [x] Update documentation (README, docs/) to reference the consolidated
      `collab` crate API.
- [x] Review CI scripts for references to removed crates and update packaging
      or publishing workflows.
- [x] Communicate the migration (release notes, internal announcements) and
      plan for versioning the new unified crate.
