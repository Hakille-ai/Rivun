## 2026-08-14T00:27:11Z
You are Explorer M3 (Fleet Telemetry & Doctor).
Working Directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\explorer_m3
Read ORIGINAL_REQUEST.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\ORIGINAL_REQUEST.md
Read PROJECT.md at c:\Users\Stagiaire\Documents\Amadou PGC\Prs\ZAP\.agents\orchestrator\PROJECT.md

Investigate `crates/zap-telemetry`, `crates/zap-cli`, `crates/zap-node`, and existing metrics/doctor infrastructure:
1. Fleet topology discovery & node state aggregation.
2. `zap fleet doctor` CLI command implementation and health check criteria (network, storage, replay guard, journal, pack registry, certificate validity).
3. `zap incident snapshot` live process/metrics/socket/peer state capture, json/tar archive creation.
4. Prometheus exporter missing metrics (replay drops, journal rotations, segment manifest errors, pack verification failures, agent gateway requests).

Formulate a complete, detailed implementation roadmap for worker_m3 and write handoff.md in your working directory. Report your findings to parent when done.
