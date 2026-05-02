# Restate-on-fjall Migration

> **Living document.** Update the status banner, phase checkboxes, open questions,
> and findings log as the work progresses. Treat agent transcripts and ad-hoc notes
> as throw-away; this doc is the source of truth.

---

## Status

| Field | Value |
|---|---|
| Current phase | **Phase 1A** — `restate-storage-engine` crate scaffolded, no traits yet |
| Last updated | 2026-05-01 |
| Blocking issues | None for log-server. Partition-store (Phase 4) is gated on snapshot/checkpoint redesign — see §Phase 4. |
| Branch | `feat/fjall` (long-lived integration branch on `ACNoonan/restate`) |
| Latest commit on branch | `1e403e699 feat(storage-engine): scaffold backend-agnostic storage engine crate` |

**What's working today:** The PoC at [`ACNoonan/restate-fjall-poc`](https://github.com/ACNoonan/restate-fjall-poc) validates all three of Restate's RocksDB merge operators (`MetadataMerge`, `LogStateMerge`, `PartitionMerge`/VQueueMeta) on the patched coordinode-fjall fork — 32 smoke tests across the three operators, all green. The fork patches required (operator-name persistence + active-journal recovery skip-check + non-exhaustive `CompressionType` arms + typed error variant) live on `feat/*` branches at [`ACNoonan/coordinode-lsm-tree`](https://github.com/ACNoonan/coordinode-lsm-tree) and [`ACNoonan/coordinode-fjall`](https://github.com/ACNoonan/coordinode-fjall) and are PR-ready to `structured-world`.

---

## Goal

Replace Restate's RocksDB storage with [coordinode-fjall](https://github.com/structured-world/coordinode-fjall) across all four consumer crates (log-server, bifrost local_loglet, metadata-server raft, partition-store) without losing tests or production behavior. End-state: Restate compiles and runs end-to-end on fjall; RocksDB is removed entirely.

## Approach

**Trait-abstraction shim, not per-crate feature flags or hard cutover.** Concretely:

- A new `restate-storage-engine` crate (`crates/storage-engine/`) defines the backend-agnostic traits the consumers depend on (`Engine`, `Keyspace`, `WriteBatch`, `Snapshot`, `ReadIter`, `MergeOperator`).
- `restate-storage-engine-rocksdb/` implements those traits over the existing `restate-rocksdb` infra (mostly a re-export wrapper).
- `restate-storage-engine-fjall/` implements them over the patched coordinode-fjall fork.
- Each consumer crate switches imports from `rocksdb::*` (or `restate-rocksdb`) to `restate-storage-engine`, then a runtime/config flag picks the backend.

Why traits over the alternatives:

- **Hard cutover per crate** — fast for log-server alone, but no rollback path and no way to bisect a regression. Fails the moment we discover (e.g.) that fjall snapshots can't replace `Checkpoint::export_column_family` four crates in.
- **Per-crate feature flags** — duplicates ~3,000 LOC of `#[cfg(...)]` blocks per crate. Drift is inevitable; test matrix doubles; bugs only repro on one CI lane.
- **Trait shim** — more upfront work (~2,000 LOC) but pays off across all four consumers, leaves a clean rollback (flip a config flag), and produces PR-friendly diffs to upstream Restate.

Trait shapes will emerge in lockstep with the first consumer migration (log-server). Adding traits speculatively up front is guesswork; growing them against real call sites produces the right shape on the first try.

## Migration sequence

Smallest blast radius first. Each phase is 1+ PRs to the `feat/fjall` integration branch.

| # | Crate | Why this order | Key risk |
|---|---|---|---|
| 1 | `crates/log-server/` | Smallest (2 CFs, 1 merge op). Admin-frequency writes. Isolated tests. The `MetadataMerge` operator is the simplest of the three. | Best place to debug the trait shape and config round-trip without hot-path pressure. |
| 2 | `crates/bifrost/src/providers/local_loglet/` | Same shape as log-server but hot-path writes. Closely mirrors log-server's structure. | Validates the trait surface holds under load. |
| 3 | `crates/metadata-server/src/raft/storage/` | Two CFs, no merge op. ~1,000 LOC. Mechanical at this point. | Proves the shim handles a third independent consumer. |
| 4 | `crates/partition-store/` | 1 DB per partition, ~12 CFs, vqueue merge op + `Checkpoint::export_column_family` snapshot machinery. | **Snapshot redesign required.** Don't start until §Phase 4 has a concrete prototype answer. |

`crates/rocksdb/` (the shared infra wrapper) stays RocksDB-only; `restate-storage-engine-rocksdb` is a thin layer over it. Don't try to migrate `restate-rocksdb` itself — it's the wrong abstraction boundary.

`crates/vqueues/` migrates implicitly with partition-store (depends on it for shared infra; merge op already lives in partition-store).

---

## Repository layout

Four repos involved. Two of them — the coordinode forks — carry patches we need; one is our PoC; one is the consumer.

| Repo | Branch | Purpose | Pushed |
|---|---|---|---|
| `ACNoonan/restate` (fork of `restatedev/restate`) | `feat/fjall` | The migration work itself. Integration branch for all topic branches. | ✅ |
| `ACNoonan/restate-fjall-poc` | `main` | Standalone smoke tests for the three merge operators. Validation, not implementation. | ✅ |
| `ACNoonan/coordinode-lsm-tree` (fork) | `feat/merge-operator-name-persistence` | 2-commit patch: persist `MergeOperator::name()` in manifest, error on mismatch at open. PR-ready to structured-world. | ✅ |
| `ACNoonan/coordinode-fjall` (fork) | `feat/restate-merge-operator-fixes` | 3-commit patch: typed error variant + non-exhaustive CompressionType arms + active-journal recovery skip-check. PR-ready to structured-world. | ✅ |
| `ACNoonan/coordinode-fjall` | `local/poc-pin` | Local-only Cargo.toml path-pin for PoC dev. **Not pushed.** Will be obsolete once structured-world cuts a release that includes our patches. | ❌ (intentional) |

Local sibling layout this work assumes:

```
restatedev/
├── restate/                          (consumer; ACNoonan fork)
├── restate-fjall-poc/                (validation; ACNoonan repo)
├── coordinode-fjall/                 (engine wrapper; ACNoonan fork; on local/poc-pin)
└── coordinode-lsm-tree-v4.1.0/       (engine; worktree of fork at v4.1.0;
                                       on feat/merge-operator-name-persistence)
```

---

## Phase 0 — Prerequisites (✅ done)

- [x] Validate all three Restate merge operators work on coordinode-fjall via PoC
- [x] Patch coordinode-lsm-tree with operator-name persistence
- [x] Patch coordinode-fjall with typed error variant + recovery skip-check + non_exhaustive arms
- [x] Push all four repos to `ACNoonan` on GitHub
- [x] Fork `restatedev/restate` to `ACNoonan/restate` and create `feat/fjall` branch

---

## Phase 1 — log-server migration

The first consumer migration. Drives the trait surface; everything that comes after reuses what we learn here.

**Source:** `crates/log-server/src/rocksdb_logstore/` — 5 files (`builder.rs`, `store.rs`, `writer.rs`, `keys.rs`, `metadata_merge.rs`, `mod.rs`, `record_format.rs`, `error.rs`). ~3,000 LOC total.

**Public boundary:** The `LogStore` trait (`logstore.rs:108`). No `rocksdb::*` types leak past this trait, which makes the migration much easier — we only need to keep this trait's implementation working, not change its callers.

**Merge operator:** `MetadataMerge` (registered at `builder.rs:391`). Already ported to the PoC as `TrimPointMerge`; lift directly when implementing the fjall adapter.

### Phase 1A — scaffold the storage-engine crate (✅ done)

- [x] Create `crates/storage-engine/` with `Cargo.toml`, `lib.rs`, `error.rs`
- [x] Add to workspace members + `workspace.dependencies`
- [x] `Error` enum with typed `MergeOperatorMismatch { stored, supplied: Option<String> }` variant (both backends surface this)
- [x] `cargo check -p restate-storage-engine` clean
- [x] `cargo hakari generate` clean (no workspace-hack changes needed)
- [x] Committed on `feat/fjall`

### Phase 1B — design and implement the trait surface

Drive the trait surface against log-server's actual call sites — see §Trait shape exploration below. Implement and ship in lockstep with the rocksdb adapter (Phase 1C) so we get immediate validation that the surface compiles against a real backend.

- [ ] Trait: `Engine` — open(config) → Engine; list keyspaces; close
- [ ] Trait: `Keyspace` — handle to a CF/keyspace; create/get; per-keyspace options
- [ ] Trait: `WriteBatch` — `put`, `merge`, `delete_range`, atomic `commit(write_options)`
- [ ] Trait: `ReadIter` — `seek`, `seek_for_prev`, `next`, `valid`, `key`, `value`; lower/upper bounds; prefix mode; `fill_cache`/`async_io` flags as backend hints
- [ ] Trait: `BatchedMultiGet` — multi-key point lookup (log-server uses this in `load_loglet_state`)
- [ ] Trait: `MergeOperator` — re-export from `restate-storage-engine` (mirror PoC contract: `name()` + `merge(key, base, operands)`)
- [ ] Snapshot trait — log-server doesn't use snapshots, so spec but don't implement until Phase 2 needs it
- [ ] Async vs sync: backends will need async writes (log-server's `write_batch` is `.await`'d). Trait API is async.
- [ ] **Acceptance:** trait crate compiles standalone; `cargo doc` produces useful docs; no `rocksdb::*` or `fjall::*` types in the trait signatures.

### Phase 1C — `restate-storage-engine-rocksdb` adapter

- [ ] Create `crates/storage-engine-rocksdb/`
- [ ] Implement traits over existing `restate-rocksdb::RocksAccess`, `RawRocksDb`, `BoundColumnFamily`
- [ ] Map `WriteBatch` → `rocksdb::WriteBatch`
- [ ] Map `ReadIter` → `RocksIterator` (already wrapped in `restate-rocksdb`)
- [ ] Map `BatchedMultiGet` → `batched_multi_get_cf_opt`
- [ ] Map `MergeOperator` → `set_merge_operator` registration with FFI shim
- [ ] **Acceptance:** `cargo check -p restate-storage-engine-rocksdb` clean; round-trip a write+read+iter against a real RocksDB tempdir in a unit test

### Phase 1D — migrate log-server to consume traits

- [ ] Modify `rocksdb_logstore/builder.rs`: replace `rocksdb::*` imports with trait imports; `Arc<BoundColumnFamily>` → `Arc<dyn Keyspace>`
- [ ] Modify `rocksdb_logstore/store.rs`: same
- [ ] Modify `rocksdb_logstore/writer.rs`: same
- [ ] Modify `rocksdb_logstore/metadata_merge.rs`: register through trait
- [ ] Rename `rocksdb_logstore/` → `logstore/` (no longer backend-specific)
- [ ] Update `lib.rs` `mod` declarations
- [ ] **Acceptance:** `cargo nextest run -p restate-log-server` passes (still on RocksDB through the new adapter); ship as Phase A PR to `feat/fjall`

### Phase 1E — `restate-storage-engine-fjall` adapter

- [ ] Create `crates/storage-engine-fjall/`
- [ ] Add git deps to `ACNoonan/coordinode-{lsm-tree,fjall}` on the patched branches (use `git = "..."` not `path = "..."` so the crate is buildable for anyone with a fresh clone)
- [ ] Implement traits over `coordinode-fjall::{Database, Keyspace, Batch, Iter}`
- [ ] Lift `TrimPointMerge` from PoC as canonical `MetadataMerge` impl
- [ ] Implement `BatchedMultiGet` (fjall doesn't have a batched API; loop over `get` and document the perf characteristic)
- [ ] **Acceptance:** `cargo check -p restate-storage-engine-fjall` clean; round-trip write+read+iter against a fjall tempdir

### Phase 1F — backend selector + differential harness

- [ ] Add `LogStoreBackend` enum in `log-server/src/service.rs` (rocksdb | fjall) selected at construction
- [ ] Add `--storage-engine=rocksdb|fjall` flag (or config field). Default `rocksdb` in this PR.
- [ ] Differential harness in `crates/storage-engine/tests/differential.rs`:
  - Lift the 6 `metadata_merge` tests from `restate-fjall-poc/tests/metadata_merge.rs`
  - Run identical operations against both backends via the trait API
  - Assert identical post-state (counter values, EMA values, on-disk read results — *not* on-disk byte-equality, the file formats differ by design)
- [ ] **Acceptance:** `cargo nextest run -p restate-log-server` passes against both backends; differential harness green; ship as Phase B PR

### Phase 1G — bench + flip default

- [ ] Capture `tools/logserver-bench` baseline on RocksDB
- [ ] Re-run on fjall, document deltas
- [ ] Flip log-server config default to `fjall`
- [ ] Re-run full test matrix on the integration branch
- [ ] **Acceptance:**
  - `cargo nextest run -p restate-log-server` clean on `--storage-engine=fjall`
  - `cargo nextest run --all-features` clean (other crates still on RocksDB)
  - `cargo fmt --all -- --check`, `cargo clippy --all-features --all-targets --workspace -- -D warnings`, `cargo deny --all-features check` clean (per `CLAUDE.md` lines 63–65)
  - Bench within **15%** on writes, **25%** on mixed read/write/trim. Bigger regressions allowed only with documented reason + follow-up issue.
  - Manual: bring up single-node Restate locally, send invocations, confirm logs persist + replay across process restart.
  - Ship Phase C PR. **Phase 1 milestone: log-server is on fjall.**

### Trait shape exploration

Drive trait design from these specific log-server call sites (file:line):

- **Open & configure DB:** `crates/log-server/src/rocksdb_logstore/builder.rs:38-62, 92-146`
- **CF definitions:** `builder.rs:243-348` (data CF), `builder.rs:362-392` (metadata CF)
- **Merge operator registration:** `builder.rs:391`
- **Point-multi-get on metadata:** `store.rs:202-310` (`load_loglet_state`)
- **Forward range scan with bounds + prefix:** `store.rs:335-477` (`read_records`)
- **Reverse seek to find tail:** `store.rs:271-273` (`seek_for_prev`)
- **WriteBatch composition:** `writer.rs:409-479` (`store`/`trim`)
- **Per-batch sync override:** `writer.rs:494-505` (`WriteOptions::set_sync`, `disable_wal`)

Iterator option flags log-server actually uses (the trait must expose these as backend hints, even if some backends ignore them):

- `set_tailing(false)`
- `set_prefix_same_as_start(true)`
- `fill_cache(false)`
- `set_async_io(true)`
- `set_iterate_lower_bound(...)`
- `set_iterate_upper_bound(...)`

Configuration surface log-server exposes (15 keys at `crates/types/src/config/log_server.rs:34-305`) maps to engine config; either the trait crate exposes a typed config struct or backends define their own and we wire log-server's keys through the backend constructor.

---

## Phase 2 — bifrost local_loglet migration

**Source:** `crates/bifrost/src/providers/local_loglet/` — file structure mirrors log-server's `rocksdb_logstore/` very closely. Most of what we built for Phase 1 transfers directly.

**Merge operator:** `LogStateMerge` (already in PoC). Hot path — fires on every log write.

**Key differences from log-server:**

- Uses prefix extractors heavily (per-loglet ID prefix). Trait must expose prefix-iterator semantics. Fjall's range iterators emulate this without RocksDB's prefix bloom optimization — measure perf impact.
- Single DB but the loglet state metadata CF gets a lot more write traffic than log-server's metadata CF.
- Already imports `BlockBasedOptions, Cache, SliceTransform` per-CF — these need a fjall-friendly mapping.

**Sequence (after Phase 1 lands):**

- [ ] 2A: Migrate `bifrost/src/providers/local_loglet/log_store.rs` to consume traits
- [ ] 2B: Migrate `log_store_writer.rs` to consume traits
- [ ] 2C: Add fjall backend to bifrost (`LocalLogletBackend` selector mirroring log-server's `LogStoreBackend`)
- [ ] 2D: Lift `LogStateMerge` from PoC as canonical impl in `restate-storage-engine-fjall`
- [ ] 2E: Differential harness covering `LogStateMerge` (lift from PoC's 9 `log_state_merge` tests)
- [ ] 2F: Bench bifrost write throughput on both backends, document deltas
- [ ] 2G: Flip default
- [ ] **Acceptance:** `cargo nextest run -p restate-bifrost` clean on both backends; bench within 20% on write throughput

Estimated 8-10 tasks total.

---

## Phase 3 — metadata-server raft migration

**Source:** `crates/metadata-server/src/raft/storage/` (~1,000 LOC, single file `rocksdb.rs`).

**No merge operator.** Two CFs: raft log + raft HardState. Mechanical port at this point — most of the trait surface is already validated by Phases 1 and 2.

**Sequence:**

- [ ] 3A: Migrate `raft/storage/rocksdb.rs` to consume traits
- [ ] 3B: Add fjall backend selector
- [ ] 3C: Validate raft persistence semantics under crash (kill-9 mid-write tests if they exist; if not, add)
- [ ] 3D: Bench raft append throughput on both backends
- [ ] 3E: Flip default
- [ ] **Acceptance:** `cargo nextest run -p restate-metadata-server` clean; raft log replay works across process restart

Estimated 5-8 tasks total. Confidence-builder before partition-store.

---

## Phase 4 — partition-store migration

**Source:** `crates/partition-store/` — 1 DB per partition, ~12 CFs, the `PartitionMerge` operator (vqueue stats with EMAs and stage-transition counters).

**This phase is gated on a snapshot redesign.** Restate's partition snapshots use RocksDB's `Checkpoint::export_column_family` (`crates/rocksdb/src/lib.rs:544`) to ship physical SST files to object storage. Fjall snapshots are read-only views, not exportable file sets. There is no direct fjall analog.

### Snapshot/checkpoint redesign — required prework

Pick one before starting Phase 4 implementation:

**Option A: Logical export.** Iterate the keyspace at a snapshot seqno, write KV pairs to object storage in a custom format. Restore reads back the KV stream and bulk-loads via fjall's `Ingestion` API. Pros: backend-agnostic (could switch storage engines again later without redoing this); doesn't depend on fjall internals. Cons: slower than file-shipping (must walk + decompress + recompress); larger snapshot artifacts on the wire.

**Option B: Use fjall's SST file format directly.** Fjall stores SSTs in a documented format. Snapshots could ship them directly, akin to RocksDB's checkpoint mechanism. Pros: matches existing perf profile. Cons: tight coupling to fjall internals; if structured-world changes the format, we re-do this; no easy backwards/forwards compat.

**Option C: Hybrid.** Logical export for cross-version restore safety, file-shipping for same-version replicas. Most code; most flexibility.

**Decision needed before Phase 4 starts.** Recommend prototyping Option A on a small partition first; if perf is acceptable, ship it.

### Phase 4 sequence (after snapshot decision)

- [ ] 4A: Implement chosen snapshot strategy in `restate-storage-engine-fjall`
- [ ] 4B: Migrate `partition-store/src/partition_db.rs` to consume traits
- [ ] 4C: Migrate per-table modules (`vqueue_table/`, `inbox_table/`, `journal_table/`, etc.) — each touches one or more CFs
- [ ] 4D: Lift `VQueueMetaMerge` from PoC as canonical impl
- [ ] 4E: Add fjall backend selector
- [ ] 4F: Differential harness covering `PartitionMerge` (lift from PoC's 17 `vqueue_meta_merge` tests, especially the partial-merge associativity test)
- [ ] 4G: Validate snapshot create + restore on a real partition
- [ ] 4H: Bench partition-store on both backends — vqueue write rate, snapshot create time, snapshot restore time
- [ ] 4I: Flip default
- [ ] **Acceptance:** `cargo nextest run -p restate-partition-store` clean; manual: create a partition, run a vqueue workload, snapshot it, restore on a fresh node, confirm state matches

Estimated 15-20 tasks. **Largest single phase.**

---

## Phase 5 — remove RocksDB

After all four consumers are on fjall by default:

- [ ] Mark `rocksdb` config option as deprecated; warn on startup if any consumer is configured for rocksdb
- [ ] Internal soak period: run on fjall in dev for ≥ 2 weeks, shake out crashes/perf regressions
- [ ] Remove `restate-storage-engine-rocksdb`
- [ ] Remove `restate-rocksdb`
- [ ] Remove `rocksdb` workspace dep
- [ ] **Acceptance:** `cargo tree | grep -i rocksdb` returns empty; CI green

---

## Cross-cutting concerns

### Test strategy

Per the plan agent's recommendation:

- **Primary signal:** existing `cargo nextest run -p restate-<crate>` per-crate suite. Restate's crate-level tests are fast and exercise the storage path through real consumer code. If they pass on RocksDB-via-trait and on fjall-via-trait with no consumer-code changes, the seam is right.
- **Differential harness:** for each merge operator only. Lift the PoC's 32 tests into `crates/storage-engine/tests/differential.rs`. Run against both backends, assert identical post-state via the trait API. Catches merge-op semantic drift, the highest-risk category.
- **Reject byte-equality of on-disk state.** Different engines, different file formats; chasing this is a sink.
- **Integration tests last.** `cargo nextest run --all-features` only on the integration branch, not on every topic-branch push. Accept up to 2-3 backend-sensitive integration tests being marked `#[ignore]` with tracked follow-ups during the transition.
- **Performance:** `tools/logserver-bench` for log-server, `tools/bifrost-benchpress` for bifrost, `tools/pp-bench` for partition-store. Capture baselines on RocksDB before each phase starts.

### Performance gates per phase

| Phase | Crate | Tool | Acceptable regression |
|---|---|---|---|
| 1 | log-server | `tools/logserver-bench` | 15% writes, 25% mixed |
| 2 | bifrost | `tools/bifrost-benchpress` | 20% write throughput |
| 3 | metadata-server raft | TBD (custom harness if needed) | 25% raft append latency |
| 4 | partition-store | `tools/pp-bench` | 25% per-request, 50% snapshot create/restore |

Bigger regressions allowed only with documented justification + follow-up issue tracked here in §Findings log.

### Workspace-hack maintenance

After every `Cargo.toml` change run `cargo hakari generate` (per `CLAUDE.md` line 67). The new `restate-storage-engine` crate is wired correctly; subsequent crate additions need the same treatment.

### Upstream PR strategy

Two patches in `ACNoonan/coordinode-{lsm-tree,fjall}` are PR-ready to `structured-world`. Independent of the migration timeline:

- `coordinode-lsm-tree#feat/merge-operator-name-persistence` — 2 commits; could PR now.
- `coordinode-fjall#feat/restate-merge-operator-fixes` — 3 commits; could PR now (split into 3 separate PRs if maintainer prefers).

Decision on when/whether to PR is independent of this migration's progress. The migration depends on the patches being available on `ACNoonan/*`; whether they're also merged to `structured-world/*` is upstream housekeeping.

If `structured-world` cuts a release that includes our patches, the `ACNoonan/coordinode-fjall#local/poc-pin` branch becomes obsolete and the PoC + Phase 1E adapter can switch from git deps on `ACNoonan/*` to crates.io deps.

---

## Open questions

| # | Question | Resolution path |
|---|---|---|
| Q1 | What's the right async story for the trait? Restate uses tokio extensively; restate-rocksdb's `write_batch` is async. Trait API should match. | Default to `async fn` in the trait; backends that need to block use `spawn_blocking` internally. Validate during Phase 1B. |
| Q2 | Does fjall's keyspace-per-CF model handle Restate's 12-CF partition store cleanly? Or do we need to multiplex? | Test in Phase 4 prototype. If 12 keyspaces per partition × N partitions blows up file handles or memory, multiplex via key prefix in fewer keyspaces. |
| Q3 | Does RocksDB's per-CF `prefix_extractor` have a fjall analog? | Verified no. Range iteration with computed bounds is the substitute; check perf in Phase 2 (bifrost relies on this heavily). |
| Q4 | Can fjall's `Batch` write atomically across keyspaces? | **Need to verify before Phase 1B.** Log-server writes data + metadata in one batch for crash-consistency. If fjall doesn't guarantee cross-keyspace atomicity, we either redesign or accept a small consistency window. |
| Q5 | What's fjall's snapshot lifetime/cleanup behavior? Long-lived snapshots in RocksDB pin disk space; what about fjall? | Verify in Phase 1B with a stress test (open snapshot, write 1GB, close snapshot, observe disk). |
| Q6 | Snapshot strategy for partition-store (Option A/B/C above). | Decide before Phase 4 starts. Prototype Option A on a small partition first. |
| Q7 | Background compaction observability — RocksDB exposes Priority/IoMode through `restate-rocksdb`. Fjall has different machinery. | Write equivalent counters in `restate-storage-engine-fjall`; emit through restate's metrics layer. |

Append new questions as they come up.

---

## Findings log

Append-only. Date-stamp each entry. Prior findings stay; we don't rewrite history.

### 2026-05-01

- **PoC validates the fork's API for all three Restate operators.** 32 tests across `MetadataMerge` (6), `LogStateMerge` (9), `VQueueMetaMerge` (17). The hardest case (VQueueMeta with non-idempotent EMA semantics) works via tag-discriminated encoding.
- **Operator-name persistence was a real correctness gap.** Without it, swapping in a different operator silently produces wrong values. Fixed in our `coordinode-lsm-tree#feat/merge-operator-name-persistence` patch.
- **fjall active-journal recovery had a real bug.** Already-flushed entries got re-applied on reopen. Invisible for `LogState`-style idempotent operators (max/max/OR), silent corruption for `VQueueMeta`-style non-idempotent operators (counters double on every restart). Fixed in our `coordinode-fjall#feat/restate-merge-operator-fixes` patch (3rd commit).
- **fjall's single-method `merge` API forces the user to handle the operand-vs-base shape contract internally.** RocksDB has two callbacks (`full_merge` + `partial_merge`); fjall folds them into one and stores the result as either `Value` or `MergeOperand` depending on its own iteration outcome (the merge function can't influence which). For operators where base ≠ operand shape, the user MUST use a tag-discriminated encoding. Worked out the pattern in PoC's `LogStateMerge` and reused in `VQueueMetaMerge`.
- **fjall isn't published.** `coordinode-fjall` v4.1.0 (the latest tag) isn't on crates.io and doesn't compile against any released `coordinode-lsm-tree` because the latter's `CompressionType` is `#[non_exhaustive]` and fjall's match arms have no wildcard. We use a local path-pin in the PoC and a git dep in the migration. Fixed wildcards in our patch.
- **`restate-rocksdb` is a partial wrapper, not a complete abstraction.** It handles DB lifecycle, background tasks, and perf counters, but log-server still imports `rocksdb::*` directly for write batches, iterators, read options, write options. The trait shim is filling a real gap.

---

## References

- **PoC repo:** https://github.com/ACNoonan/restate-fjall-poc
- **Restate fork (this work):** https://github.com/ACNoonan/restate/tree/feat/fjall
- **lsm-tree patches:** https://github.com/ACNoonan/coordinode-lsm-tree/tree/feat/merge-operator-name-persistence
- **fjall patches:** https://github.com/ACNoonan/coordinode-fjall/tree/feat/restate-merge-operator-fixes
- **Upstream Restate:** https://github.com/restatedev/restate
- **Upstream coordinode-lsm-tree:** https://github.com/structured-world/coordinode-lsm-tree
- **Upstream coordinode-fjall:** https://github.com/structured-world/coordinode-fjall
- **Restate `CLAUDE.md`** (validation steps, code style): `/Users/adamnoonan/Documents/restatedev/restate/CLAUDE.md`

### Key file paths in Restate source

- log-server RocksDB code: `crates/log-server/src/rocksdb_logstore/`
- bifrost local_loglet RocksDB code: `crates/bifrost/src/providers/local_loglet/`
- metadata-server raft RocksDB code: `crates/metadata-server/src/raft/storage/`
- partition-store RocksDB code: `crates/partition-store/src/`
- shared infra: `crates/rocksdb/`
- partition-store snapshot/checkpoint: `crates/rocksdb/src/lib.rs:544` (`Checkpoint::export_column_family`)
