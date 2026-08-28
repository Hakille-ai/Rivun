## 2026-08-14T01:40:11Z
Objective: Formulate the detailed technical blueprint for Milestone 1 (R1: High-Performance Durable Core & Replay Protection).
Investigate and design:
1. Disk-persisted restart-resistant durable replay store in `rivun-net` / `rivun-node` (`DurableReplayStore` saving nonces and frame fingerprints to disk e.g. sled/redb/sqlite/append-only log, recovering on restart, rejecting replay attacks after node reboot).
2. Segment rotation in `rivun-journal`: automatic trigger on segment size/count limit, sealing closed segment, computing cryptographic hash.
3. Signed segment manifest (`SignedReceiptSegmentManifest`) in `rivun-ledger`: signing closed segment manifests with node keypair, saving manifest file `.zjmanifest.json.sig`.
4. Fast indexed queries over receipt journal segments (`rivun-journal`/`rivun-ledger` index structures for timestamp/sequence/hash queries).

Write your concrete implementation plan (exact file paths, struct definitions, method signatures, crate changes) to `c:\Users\Stagiaire\Documents\Amadou PGC\Prs\rivun\.agents\teamwork_preview_explorer_m1\handoff.md` for Worker consumption. Notify orchestrator via send_message when complete.

