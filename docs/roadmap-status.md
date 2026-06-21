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
| Receipt fsync/segments/index | partial | `zap-journal`, `ReceiptJournalStore`, `ReceiptFsyncPolicy`, `[receipts] dir`, binary segments, sidecar indexes, bounded `until_processed_at_micros`, pull cursor tests | Add segment sealing/compression policy and durable peer replay windows |
| Durable replay window | partial | In-memory replay guard and datagram nonce cache | Add optional restart-persistent replay windows |
| Runtime host-call limits | partial | Host call byte limit and runtime bounds exist | Add per-action budget profiles and richer error taxonomy |

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
| Agent session/delegate/negotiate | partial | `zap agent session/delegate/negotiate`, CLI tests, session/delegation/negotiation fixtures | Add receipt links, storage, and persistent orchestration |
| Agent receipt linkage | partial | Receipts record message kind/subject and PoA | Link intent/session/capabilities/output artifacts explicitly |
| Evidence export | partial | `zap memory export-evidence` emits payload-free memory and receipt summaries | Add signed bundle manifest and optional encrypted raw evidence archive |
| Agent framework adapters | planned | Architecture docs only | Add adapters outside the wire protocol core |

## PACT Profile

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| PACT record contract | done | `crates/zap-pact`, `zap pact create/sign/verify`, `fixtures/pact-record-v1.json`, Rust tests | Keep canonical payload frozen through fixtures |
| PACT bundles and revocation | done | `ZapPactBundle`, `ZapPactRevocation`, `zap pact revoke`, `zap pact bundle verify`, fixture verifier | Add richer multi-revocation examples if deployments need them |
| PACT receipt references | done | `PactReceiptReference`, node integration test, `docs/receipts.md` | Add policy decision population when policy reporting is plumbed into receipt creation |
| PACT SDK conformance | done | Rust/TypeScript/Python/Go helpers and shared fixture tests | Add browser-lite SDK coverage when browser mode exists |

## Phase 4: Domain Packs and Marketplace

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Domain pack manifest | done | `docs/domain-packs.md`, `zap pack validate/inspect/list` | Add signing/build/install workflows |
| Preview packs | done | agentic-dev, smart-building, cloud-ops, industrial, personal-ai, healthcare, finance | Add robotics and data-platform packs |
| Pack catalog | done | `zap pack list --root ... --json` | Expose catalog in website or ZapStore |
| Pack marketplace | planned | ZapStore driver registry exists | Extend ZapStore to signed domain-pack registry |

## Phase 5: Fleet and Multi-Transport

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Static peers | done | Config, trust, peer invite/accept/rotate/revoke | Add live enrollment and revocation propagation |
| Fleet doctor | planned | `doctor --strict` local checks | Add fleet topology inspection across peers |
| Stream/gateway transports | planned | UDP transport and control messages | Add HTTP, WebSocket, gRPC, MQTT/NATS/Kafka/ROS2/OPC UA/Modbus bridges |
| Incident snapshot | partial | `zap incident snapshot` captures doctor/config/memory/receipt/cache summaries | Add live process metrics, network state, and fleet-wide peer snapshots |

## Phase 6: Architecture Modularization

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Node service boundaries | planned | `zap-node` still owns many responsibilities | Extract config, control, discovery, receipts, registry, PoA, route, runtime, observability services |
| Cross-boundary invariant tests | partial | Many unit and integration tests exist | Add direct invariant suites per service boundary |

## Phase 7: 1.0 Readiness

| Item | Status | Evidence | Remaining Work |
| --- | --- | --- | --- |
| Compatibility matrix | partial | `docs/sdks.md`, `docs/versioning.md` | Add generated protocol constant/source matrix |
| Release checklist | done | `docs/release.md`, `tools/xtask release readiness`, release workflow | Keep gates current as new SDKs and packs are added |
| External audit plan | planned | Security docs exist | Add third-party review plan and audit scope |
| Official examples strict doctor | planned | Example configs exist | Add CI that runs `doctor --strict` on official examples |

## Next Highest-Value Implementation Blocks

1. Wire receipt segment manifests into daemon log rotation and disk-backed indexes.
2. Expand fixtures to generated binary golden vectors and fixture manifests.
3. Add durable restart-persistent replay windows.
4. Add fleet topology inspection and fleet-wide incident snapshots.
5. Add signed evidence bundle manifests and optional encrypted raw evidence archive.
6. Extend ZapStore with signed domain-pack build/sign/verify/install workflows.
