# Progress Log

Last visited: 2026-08-14T01:58:50Z

- Implemented WAL tail truncation on `open()` for `DurableNonceStore` (`zap-net`) and `DurableReplayStore` (`zap-node`).
- Preserved original `node_id` during WAL `compact()` (`zap-net` and `zap-node`).
- Implemented safe `saturating_add` timestamp arithmetic in `DurableReplayStore::check_and_insert()`.
- Derived isolated per-peer WAL file paths in `ZapEndpoint::add_peer()`.
- Updated `scan_records()` and `verify()` in `zap-journal` so segment pruning (`max_segment_count`) does not cause `HashChainMismatch`.
- Fixed segment index building in `zap-ledger` to iterate over available segments, extracted true `segment_id` in `rotate_and_seal_segment()`, and auto-signed closed segment manifests in `ReceiptJournalStore::append()`.
- Fixed all `clippy::manual_is_multiple_of` lints and test assertions in `m1_journal_stress.rs` and `m1_challenger_stress.rs`.
- Ran `cargo test -p zap-net -p zap-node -p zap-journal -p zap-ledger` (Passed 100%).
- Ran `cargo clippy -p zap-net -p zap-node -p zap-journal -p zap-ledger --all-targets -- -D warnings` (Passed clean).
