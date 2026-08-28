# Progress Log

Last visited: 2026-08-14T01:58:50Z

- Implemented WAL tail truncation on `open()` for `DurableNonceStore` (`rivun-net`) and `DurableReplayStore` (`rivun-node`).
- Preserved original `node_id` during WAL `compact()` (`rivun-net` and `rivun-node`).
- Implemented safe `saturating_add` timestamp arithmetic in `DurableReplayStore::check_and_insert()`.
- Derived isolated per-peer WAL file paths in `ZapEndpoint::add_peer()`.
- Updated `scan_records()` and `verify()` in `rivun-journal` so segment pruning (`max_segment_count`) does not cause `HashChainMismatch`.
- Fixed segment index building in `rivun-ledger` to iterate over available segments, extracted true `segment_id` in `rotate_and_seal_segment()`, and auto-signed closed segment manifests in `ReceiptJournalStore::append()`.
- Fixed all `clippy::manual_is_multiple_of` lints and test assertions in `m1_journal_stress.rs` and `m1_challenger_stress.rs`.
- Ran `cargo test -p rivun-net -p rivun-node -p rivun-journal -p rivun-ledger` (Passed 100%).
- Ran `cargo clippy -p rivun-net -p rivun-node -p rivun-journal -p rivun-ledger --all-targets -- -D warnings` (Passed clean).

