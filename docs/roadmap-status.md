# Roadmap Implementation Status

This file tracks implementation evidence for the roadmap. It is intentionally
stricter than `roadmap.md`: an item is marked done only when the repository has
code, tests, docs, and operator evidence for the promised behavior.

Status values:

- `done`: implemented, tested, and documented.
- `partial`: useful work exists, but the roadmap promise is not fully met.
- `planned`: documented but not implemented.

## Phase 0: Promise, Packaging, and Adoption

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Install docs | done | `docs/install.md`, website install page | Keep release artifact instructions current |
| Five-minute source quickstart | partial | `docs/getting-started.md`, `README.md` | Add exact expected terminal output for every step |
| Website docs coverage | partial | Website pages for install, agent protocol, domain packs, message policy, observability, RFC/ZEP | Add release/governance/community pages and link checking |
| GitHub contribution flow | done | Issue templates, PR template, `CODEOWNERS`, `docs/rfc-process.md` | Add examples of accepted ZEPs once proposals exist |

## Phase 1: Production Hardening

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Node metrics text | done | `ZapNode::metrics_snapshot()`, `ZapNode::metrics_prometheus_text()`, optional `[observability].http_bind` `/metrics`, `node_observability_http_serves_metrics_and_healthz` | Keep Prometheus assets aligned with emitted names |
| Health endpoint | done | `ZapNode::health_snapshot()`, `ZapNode::health_json()`, `ZapNode::healthz_text()`, optional `/healthz` and `/healthz.json`, `doctor` observability check | Add fleet-level health aggregation |
| Prometheus/Grafana assets | done | `crates/zap-ops/config`, `docs/observability.md`, `crates/zap-ops/tests/configs.rs` | Add new rules only with emitted-metric validation |
| Fail-closed message policy | done | `message_policy.default_decision`, policy tests, docs | Add more production profile examples |
| Receipt fsync/segments/index | done | `zap-journal`, `ReceiptJournalStore`, `ReceiptFsyncPolicy`, `[receipts] dir`, binary segments, sidecar indexes, bounded `until_processed_at_micros`, pull cursor tests, signed segment manifests, batch seals, MMR build | Add per-profile compression policy and fleet-level journal supervision |
| Durable replay window | done | `DurableNonceStore` WAL (`ZAPNONC1`) in `zap-net`, `DurableReplayStore` frame fingerprints in `zap-node`, stress tests across restart floods | Tune window size policy per deployment |
| Runtime host-call limits | partial | Host call byte limit and runtime bounds exist; async pipeline, streaming buffers, IPC, and opt-in ABI-compatible async node dispatch are implemented (`zap-runtime`, `zap-node`) | Add per-action budget profiles and richer error taxonomy |

## Phase 2: Protocol Spec and SDK Conformance

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Shared fixtures | partial | `fixtures/`, agent session/delegation/negotiation fixtures, PACT record/bundle fixtures, `fixtures/protocol/`, Rust/Python/TypeScript/Go tests | Add more binary golden vectors and generated fixture manifests |
| `zap fixtures verify` | done | CLI command, SDK-path conformance mode, fixture tests | Keep SDK coverage requirements current as profiles are added |
| SDK matrix | partial | `docs/sdks.md`, SDK README updates, PACT fixture verification across Rust/TypeScript/Python/Go | Expand SDKs to broader frame signing, receipt verification, and capability helpers |
| `zap schema export` | partial | CLI export of compiled protocol constants, agent schema, PACT schema/constants, control subjects, expanded fixture catalog | Add external domain-pack schema registry and SDK-generated schema parity |

## Phase 3: Agent Gateway

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Agent intent/status/result | done | `zap-agent`, `zap agent intent/status/result`, tests, docs | Keep SDK fixture coverage in sync |
| Agent session/delegate/negotiate | done | `zap agent session/delegate/negotiate`, CLI tests, session/delegation/negotiation fixtures, gateway REST+SSE endpoints and MCP tools | Add persistent orchestration state and cross-node session recovery |
| Agent receipt linkage | done | Receipts record message kind/subject and PoA; provenance chain covers intent→negotiation→policy→driver→poa→receipt with root signing | Link provenance digests into receipt records and incident evidence |
| Evidence export | partial | `zap memory export-evidence` emits payload-free memory and receipt summaries; `zap incident snapshot` bundles redacted evidence; signed bundle manifests supported | Add optional encrypted raw evidence archive |
| Agent framework adapters | partial | `zap-gateway` MCP server (stdio/HTTP), REST/SSE/WebSocket transports, provenance chain engine | Add adapters outside the wire protocol core (gRPC, NATS/Kafka bridges) |

## PACT Profile

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| PACT record contract | done | `crates/zap-pact`, `zap pact create/sign/verify`, `fixtures/pact-record-v1.json`, Rust tests | Keep canonical payload frozen through fixtures |
| PACT bundles and revocation | done | `ZapPactBundle`, `ZapPactRevocation`, `zap pact revoke`, `zap pact bundle verify`, fixture verifier | Add richer multi-revocation examples if deployments need them |
| PACT receipt references | done | `PactReceiptReference`, node integration test, `docs/receipts.md` | Add policy decision population when policy reporting is plumbed into receipt creation |
| PACT SDK conformance | done | Rust/TypeScript/Python/Go helpers and shared fixture tests | Add browser-lite SDK coverage when browser mode exists |
| Dispute-state durability | partial | `DisputeEngine::save_to_path/load_from_path`, fsynced atomic snapshots, checksum and state-invariant validation tests | Wire snapshots into cross-node orchestration and verify distributed arbitration signatures |

## Phase 4: Domain Packs and Marketplace

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Domain pack manifest | done | `docs/domain-packs.md`, `zap pack validate/inspect/list` | Keep signing/build/install workflows covered by CI |
| Preview packs | done | agentic-dev, smart-building, cloud-ops, industrial, personal-ai, healthcare, finance | Add robotics and data-platform packs |
| Pack catalog | done | `zap pack list --root ... --json` | Expose catalog in website or ZapStore |
| Pack lifecycle (build/sign/install/audit) | done | `zap pack init/build/sign/verify/install/audit`, `zap-pack`/`zap-store` bundle machinery, `zap-store/tests/pack_tests.rs` | Add signed publication and revocation across peers |

## Phase 5: Fleet and Multi-Transport

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Static peers | done | Config, trust, peer invite/accept/rotate/revoke | Add live enrollment and revocation propagation |
| Fleet doctor | done | `zap fleet doctor` aggregates 6 core criteria across nodes; `FleetDoctor` in `zap-telemetry` | Add fleet-wide health dashboards and alerting wiring |
| Stream/gateway transports | partial | `zap-gateway`: HTTP REST, SSE, WebSocket, MCP stdio; encrypted UDP transport | Add gRPC, MQTT/NATS/Kafka/ROS2/OPC UA/Modbus bridges |
| Incident snapshot | done | `zap incident snapshot` captures doctor/config/memory/receipt/cache summaries, process state, and redaction; tar and JSON output | Add fleet-wide peer snapshots aggregation |

## Phase 6: Architecture Modularization

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Node service boundaries | partial | `zap-node` gained actor modules (udp_rx, gossip, mesh, consensus, execution) and `zap-gateway`/`zap-telemetry` services | Extract config, control, discovery, receipts, registry, PoA, route, runtime, observability services further |
| Cross-boundary invariant tests | partial | Many unit and integration tests exist | Add direct invariant suites per service boundary |

## Phase 7: 1.0 Readiness

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Compatibility matrix | partial | `docs/sdks.md`, `docs/versioning.md` | Add generated protocol constant/source matrix |
| Release checklist | done | `docs/release.md`, `tools/xtask release readiness`, release workflow | Keep gates current as new SDKs and packs are added |
| External audit plan | planned | Security docs exist | Add third-party review plan and audit scope |
| Official examples strict doctor | planned | Example configs exist | Add CI that runs `doctor --strict` on official examples |

## Next Highest-Value Implementation Blocks

1. Add per-profile journal compression policy and fleet-level journal supervision.
2. Expand fixtures to generated binary golden vectors and fixture manifests.
3. Tune durable replay window sizing and rotation policy per deployment.
4. Add fleet-wide incident snapshot aggregation and topology-driven alerting.
5. Add signed pack publication and revocation propagation across peers.
6. Add gRPC and MQTT/NATS/Kafka bridge gateways on top of `zap-gateway`.
