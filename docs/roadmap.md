# Roadmap

rivun is evolving from a secure low-latency message protocol into a universal
trust fabric for agents, machines, services, and human-supervised automation.
The goal is not to make rivun a vague general-purpose platform. The goal is to
make every important action in a distributed system typed, signed, policy-gated,
sandboxed when needed, observable, and auditable after the fact.

This roadmap is intentionally ambitious. It defines the path from the current
pre-1.0 foundation to a production-grade protocol and ecosystem that can be used
across AI agents, robotics, industrial control, edge systems, cloud operations,
personal automation, healthcare, finance, and other safety-sensitive domains.

## Product Thesis

rivun should become the accountable execution layer for autonomous software and
real-world automation:

- typed intent before execution;
- cryptographic identity for every node and agent;
- deterministic policy before authority is granted;
- explicit capability discovery without implicit trust;
- Proof-of-Action for high-risk operations;
- sandboxed drivers for untrusted extensions;
- signed receipts and hash-chained memory for audit;
- portable SDKs and gateways for broad adoption;
- domain packs that make rivun useful in many industries without weakening the
  core safety model.

## Current Foundation

Already implemented:

- strict `@@rivun_HEADER@@` wire frame parsing and encoding;
- `ZENV` universal envelopes for actions, events, data, commands, queries,
  responses, streams, and control messages;
- Ed25519 node identity, frame signing, verification, and PoA certificates;
- encrypted UDP transport with static peers and replay protection;
- peer trust contracts for send, receive, forward, expiry, and key rotation;
- deterministic message policy for allow, deny, PoA, grant, human approval, and
  simulation gates;
- Wasmtime driver sandboxing with fuel, memory, timeout, output, and scoped host
  call limits;
- signed driver manifests, local RivunStore registries, publications, install
  plans, and offline bundles;
- PACT signed action records with canonical BLAKE3 hashing, Ed25519 signatures,
  revocation evidence, offline bundles, and receipt references;
- capability advertisements, grants, requirements, route planning, and
  hash-chained capability cache verification;
- append-only memory and signed receipt ledgers;
- CLI workflows for config validation, doctor checks, peers, trust, registry,
  capabilities, routes, memory, receipts, schemas, policy, and PoA;
- SDK previews for Rust, TypeScript, Python, and Go;
- CI, tests, benches, Docker packaging, documentation, and an initial website.

## North-Star Outcomes

rivun reaches the next maturity level when these outcomes are true:

- a new developer can install rivun, run a two-node demo, send a typed action, and
  verify a receipt in less than five minutes;
- production operators can run `doctor --strict` and trust that the daemon,
  policy, registry, receipts, routes, PoA, and observability posture are checked;
- every official SDK passes the same conformance fixtures for frames, envelopes,
  signatures, datagrams, control messages, receipts, capabilities, PoA, and
  PACT records;
- high-risk actions are fail-closed by policy and cannot run without explicit
  grants, PoA, human approval, or simulation when configured;
- rivun nodes expose real metrics and health status matching the Prometheus and
  Grafana assets in the repository;
- RivunStore can distribute signed drivers and domain packs through online and
  offline workflows with rollback and revocation;
- AI agent workflows can express intent, negotiate capabilities, delegate work,
  execute actions, and produce terminal receipts without relying on hidden
  natural-language interpretation inside the protocol;
- domain packs make rivun practical for concrete verticals such as agentic
  development, smart buildings, industrial control, cloud operations, personal
  AI, healthcare, and finance.

## Execution Model

The roadmap is split into delivery tracks so the project can grow without
turning into one giant rewrite. Each track should ship independently, keep
tests close to the behavior it changes, and leave operators with something
usable at the end of every milestone.

### Track A: Trust Core

Purpose: keep the protocol secure, deterministic, and auditable.

Owns:

- frame, envelope, datagram, signature, and PoA compatibility;
- message policy, trust contracts, grants, and fail-closed production defaults;
- receipts, memory chains, replay windows, and evidence export;
- protocol fixtures and SDK conformance gates.

Definition of done:

- every externally visible format has a fixture;
- every high-risk path has an explicit policy decision and receipt trail;
- compatibility breaks require a migration note and fixture update;
- production profiles can prove that no critical action uses ambient authority.

### Track B: Runtime and Fleet

Purpose: make nodes reliable in real deployments.

Owns:

- node orchestration, daemon lifecycle, health, metrics, and tracing;
- routing, forwarding, capability cache, validators, and fleet topology;
- driver runtime isolation, host ABI budgets, artifact transfer, and rollback;
- transport expansion for streams and gateways.

Definition of done:

- `doctor --strict` catches unsafe runtime and fleet posture before launch;
- metrics dashboards are backed by emitted daemon metrics;
- staged rollout and rollback paths are tested with receipts;
- runtime failures are classified, bounded, and visible to operators.

### Track C: Adoption Surface

Purpose: make rivun easy to install, understand, integrate, and extend.

Owns:

- CLI ergonomics, quickstarts, website docs, and release packaging;
- SDKs for Rust, TypeScript, Python, Go, and future languages;
- domain packs, examples, gateways, and marketplace workflows;
- contributor onboarding, RFC/ZEP process, and release communication.

Definition of done:

- a new user can complete a five-minute demo without reading internals;
- official examples run in CI and stay linked from docs;
- domain packs validate before publication;
- SDK releases are blocked by shared conformance fixtures.

## Priority Ladder

Use this ladder when deciding what to build next:

1. Safety invariants that prevent unsafe authority or broken audit trails.
2. Conformance fixtures that prevent protocol drift across SDKs.
3. Install, quickstart, and example flows that reduce adoption friction.
4. Observability and production checks that make deployments operable.
5. Domain packs and gateways that prove rivun works outside the core repo.
6. Performance work guided by measured bottlenecks and regression budgets.
7. Developer-experience polish once the underlying workflow is reliable.

This ordering matters. A new feature that expands reach should also strengthen
identity, policy, receipts, observability, or conformance. If it does not, it
should be treated as an integration experiment until those guarantees exist.

## Roadmap Phases

### Phase 0: Promise, Packaging, and Adoption Baseline

Goal: make the project immediately understandable, installable, and credible.

Deliverables:

- define one canonical tagline and product explanation across README, website,
  docs, and release notes;
- add "Use rivun when..." and "Do not use rivun when..." guidance;
- publish a clear comparison with MQTT, NATS, gRPC, Kafka, and generic service
  meshes;
- create a five-minute quickstart with expected terminal output;
- add binary install instructions, Docker image instructions, and release
  verification instructions;
- add website pages for install, security, releases, governance, community, and
  benchmarks;
- sync website docs with repository docs to avoid drift;
- add issue templates, PR template, CODEOWNERS, contributor map, and an RFC/ZEP
  process for protocol, crypto, and ABI changes.

Success metrics:

- first-run demo works on Linux, macOS, and Windows;
- website hero includes install command, GitHub link, docs link, and latest
  release status;
- docs site links every core repo doc;
- release artifacts include checksums and verification steps.

### Phase 1: Production Hardening

Goal: convert existing safety primitives into continuous production signals and
fail-closed deployment gates.

Deliverables:

- add daemon `/metrics` and `/healthz` endpoints;
- emit metrics for frames sent, received, rejected, policy decisions, route
  decisions, driver execution, PoA failures, registry status, receipt failures,
  capability cache age, and replay rejection;
- ensure every Prometheus rule references a metric that the daemon actually
  exposes;
- add structured spans around receive, verify, policy, route, execute, receipt,
  and response paths without logging payloads;
- add `message_policy.default_decision = "allow" | "deny"` with compatibility
  default and production fail-closed profiles;
- add `doctor --strict` checks for production policy coverage;
- add receipt durability options: `fsync = always | interval | off`;
- add receipt segment rotation, signed segment manifests, and indexed bounded
  pulls;
- add durable replay protection for post-restart windows where topology allows
  it;
- add per-action runtime budgets, `max_host_calls`, and classified runtime error
  categories;
- add production profile checks requiring signed manifests, signed registries,
  receipts, replay protection, and strict policy coverage.

Success metrics:

- Grafana dashboards are non-empty after a smoke test;
- no critical action can run through a default allow fallback in production mode;
- pulling 500 receipts from a 1M-line log completes under a defined target;
- replay after restart is rejected inside a configurable window;
- runtime errors are classified as timeout, fuel, memory, permission, ABI,
  host-call, or guest failure.

### Phase 2: Protocol Spec and SDK Conformance

Goal: make rivun portable, testable, and safe across languages.

Deliverables:

- create a machine-readable protocol source of truth for constants, media types,
  subjects, schema versions, error codes, frame fields, datagram fields, control
  messages, and compatibility rules;
- publish golden fixtures for unsigned frames, signed frames, auth trailers, PoA
  trailers, `ZENV`, encrypted datagrams, RivunStore messages, capability messages,
  receipts, PACT records/bundles, and agent messages;
- generate or validate SDK constants from the source of truth;
- add SDK conformance tests for Rust, TypeScript, Python, and Go;
- expand common SDK surfaces:
  - envelope encode/decode;
  - frame encode/decode/sign/verify;
  - datagram encrypt/decrypt where the platform supports it;
  - control request/response helpers;
  - RivunStore registry, bundle, and install plan verification;
  - capability query and verification;
  - receipt verification and pull helpers;
  - PACT canonical hash, signature verification, and bundle verification;
  - agent protocol contracts.
- publish a compatibility matrix in `docs/sdks.md`;
- add `rivun fixtures verify --sdk <path>` and `rivun schema export` workflows.

Success metrics:

- every official SDK passes the same fixture suite;
- no critical protocol constant is hand-maintained differently per language;
- fixture failures block SDK releases;
- browser-compatible SDK mode exists for gateway/envelope workflows.

### Phase 3: Agent Gateway and Accountable AI Workflows

Goal: make rivun the typed trust boundary between AI systems and the real world.

Deliverables:

- integrate `rivun-agent` contracts into CLI, node, SDKs, receipts, and memory;
- add CLI workflows:
  - `rivun agent intent`;
  - `rivun agent session`;
  - `rivun agent delegate`;
  - `rivun agent negotiate`;
  - `rivun agent status`;
  - `rivun agent result`.
- wrap agent messages in `ZENV` with `application/rivun-agent+json`;
- link receipts to `intent_id`, `session_id`, `capabilities_used`, route
  decision, policy decision, PoA summary, output hash, and artifact references;
- define agent memory namespaces such as `agent.session`, `agent.observation`,
  `agent.plan`, `agent.fact`, `agent.artifact`, and `policy.decision`;
- add signed memory compaction records that reference source entries;
- add `rivun memory export-evidence` for audit bundles;
- build adapters for OpenAI tool workflows, MCP, LangGraph, AutoGen, CrewAI, and
  other agent frameworks without putting model-specific logic in the wire
  protocol;
- add an Agent Trace UI or report: intent -> negotiation -> policy -> route ->
  execution -> receipt -> memory.

Success metrics:

- an agent workflow can be replayed from receipts and memory references;
- a failed action shows the responsible intent, policy gate, route, and error
  category;
- model output is treated as proposed typed intent, never as automatic authority;
- agent integrations use the same schemas and fixtures as SDKs.

### Phase 4: Domain Packs and Capability Marketplace

Goal: make rivun universal by packaging safe, reusable domain knowledge.

Domain packs should include:

- capability taxonomy;
- message schemas;
- policy templates;
- route templates;
- PoA defaults;
- simulation rules;
- WASM drivers or gateway adapters;
- example configs;
- threat model notes;
- receipts and dashboard templates;
- conformance tests;
- upgrade and migration metadata.

Priority packs:

- `rivun-pack-agentic-dev`: repository, patch, test, CI, review, PR, and release
  capabilities for auditable coding agents;
- `rivun-pack-smart-building`: sensors, thermostats, lighting, locks, alarms,
  cameras, energy controls, and human/PoA gates for risky actions;
- `rivun-pack-industrial`: PLC, Modbus, OPC UA, robot, valve, motor, emergency
  stop, safety quorum, and simulation-first defaults;
- `rivun-pack-cloud-ops`: deploy, rollback, restart, scale, secret rotation,
  incident mitigation, and blast-radius limits;
- `rivun-pack-personal-ai`: calendar, email draft, files, browser actions, local
  memory, and approval gates;
- `rivun-pack-healthcare`: record queries, alert routing, medical device commands,
  privacy defaults, and strict audit;
- `rivun-pack-finance`: trade proposal, risk check, simulation, approval, execute,
  reconciliation, and regulator-friendly evidence export.

Marketplace deliverables:

- add `rivun pack init`, `build`, `sign`, `verify`, `publish`, `install`, and
  `audit`;
- extend RivunStore from driver registry to signed capability and domain-pack
  registry;
- add pack install plans that bind drivers, schemas, policies, routes, PoA
  defaults, docs, and migrations;
- support online and offline pack bundles;
- add revocation and deprecation semantics for packs;
- publish trust metadata: author, operator signature, test receipts, risk level,
  required grants, simulator coverage, and compatibility range.

Success metrics:

- two complete packs ship with examples and tests before beta;
- pack installation can be verified offline;
- a revoked pack or driver cannot be selected by automatic resolution;
- domain examples run end-to-end with receipts and policy checks.

### Phase 5: Fleet, Mesh, and Multi-Transport Runtime

Goal: operate rivun across real fleets while preserving auditability.

Deliverables:

- convert the existing Noise helper into a live enrollment and session protocol;
- keep static peers as a deterministic air-gapped mode;
- add signed revocation propagation and trust epoch handling;
- add dynamic validator discovery and signed validator-set rollout;
- add quorum policies by subject, risk level, validator class, geography, or
  operator group;
- add fleet topology graph: nodes, peers, routes, capabilities, grants,
  validator sets, registry versions, and receipt health;
- add `rivun fleet doctor`, `rivun node health`, and `rivun incident snapshot`;
- add remote bundle artifact transfer with authenticated manifests and external
  artifact-channel verification;
- add staged rollout, canary, rollback, and deprecation enforcement;
- add stream transport for large payloads, status streams, telemetry, and
  backpressure while keeping UDP datagrams for low-latency messages;
- add gateway transports:
  - HTTP for SaaS and simple integrations;
  - WebSocket for dashboards and browsers;
  - gRPC for service backends;
  - MQTT, NATS, Kafka, ROS2, OPC UA, and Modbus bridges for domain use cases.

Success metrics:

- validator set rotation can be performed without hand-editing configs;
- fleet doctor identifies stale keys, stale capability caches, broken routes,
  invalid registries, and PoA quorum gaps;
- stream transport handles long-running agent status and large artifacts without
  weakening frame-level identity and receipt trails;
- bridge gateways preserve rivun identity, policy, and audit semantics.

### Phase 6: Architecture Modularization

Goal: keep the codebase maintainable as the product grows.

Deliverables:

- reduce `rivun-node` into a small orchestrator over clearer internal services;
- split or module-bound these responsibilities:
  - config model and validation;
  - control message serving;
  - discovery and capability cache;
  - receipt service;
  - registry and bundle service;
  - PoA service;
  - route and forwarding service;
  - runtime execution service;
  - observability service.
- preserve public APIs and CLI behavior while extracting tests per boundary;
- add stronger property and integration tests around invariants that cross
  policy, routing, PoA, registry, receipts, and runtime;
- define the stable extension points for drivers, gateways, domain packs, SDKs,
  and operator services.

Success metrics:

- core node services can be tested without launching a full daemon;
- new control subjects can be added with localized changes;
- policy, route, registry, PoA, and receipt invariants have direct tests;
- internal boundaries match the docs and threat model.

### Phase 7: 1.0 Readiness

Goal: make rivun stable enough for serious external adoption.

Release gates:

- protocol compatibility matrix is published;
- golden fixtures and conformance tests are public;
- CLI reference is generated and versioned;
- config schema reference is generated and versioned;
- driver ABI stability policy is written;
- threat model is reviewed externally;
- third-party security audit plan is published;
- benchmark methodology and baseline are published;
- deprecation policy is explicit;
- migration guide exists for every breaking change;
- all official examples pass `doctor --strict`;
- website, README, docs, SDKs, and release artifacts agree on version and
  support status.

Success metrics:

- external users can build integrations using docs and SDKs without reading the
  Rust internals;
- protocol and SDK releases are gated by conformance;
- operators can verify supply chain, runtime, policy, and receipt posture before
  production rollout.

## Cross-Cutting Workstreams

### Security

- fail-closed production policy;
- durable replay windows;
- stricter key rotation and expiry posture;
- signed revocation propagation;
- SBOM and provenance for releases;
- safer defaults for registries, manifests, receipts, and drivers;
- redacted tracing by default;
- security advisories and audit process.

### Performance

- publish benchmark baselines per platform;
- track SLOs for frame parse, sign, verify, route, policy, dispatch, UDP RTT,
  receipt verification, and runtime execution;
- add scale benchmarks for 1k peers, 10k routes, 1M receipts, and large payloads;
- keep regression thresholds blocking in CI;
- document what benchmarks measure and what they do not measure.

### Observability and Operations

- metrics and health endpoints;
- production dashboards backed by real daemon metrics;
- runbooks for registry invalid, PoA failing, receipt corruption, replay spikes,
  driver failures, and capability cache staleness;
- incident snapshot bundles;
- chaos drills for release readiness.

### SDKs and Interoperability

- protocol source of truth;
- generated constants and schemas;
- conformance fixtures;
- publishable packages for Python, TypeScript, Go, and Rust;
- planned SDKs for Java/Kotlin, C#/.NET, C/C++, Swift, Dart, and browser-lite
  workflows;
- bridges for AI, IoT, industrial, cloud, data streaming, games, simulation,
  and dashboards.

### Documentation and Community

- unified positioning;
- install and quickstart pages;
- domain examples;
- "Edit this page" links;
- docs search;
- release notes and roadmap board;
- GitHub Discussions;
- good-first-issue guide;
- RFC/ZEP governance;
- architecture decision records.

## Immediate Next 20 Tasks

Status: completed items are struck through; the open items below are the
current implementation backlog.

1. ~~Unify the tagline and product explanation across README, website, and docs.~~
2. ~~Add `docs/install.md` with binary, Docker, and source install paths.~~
3. ~~Convert getting started into a five-minute quickstart with expected output.~~
4. ~~Add missing docs pages to the website navigation.~~
5. ~~Add issue templates, PR template, CODEOWNERS, and RFC/ZEP docs.~~
6. ~~Add real daemon `/metrics` and `/healthz` endpoints.~~
7. ~~Connect Prometheus rules and Grafana panels to emitted metrics.~~
8. ~~Add configurable `message_policy.default_decision`.~~
9. ~~Add production doctor checks for fail-closed policy coverage.~~
10. ~~Add receipt fsync mode, segment rotation, and indexed pull planning.~~
11. ~~Add durable replay protection design and implementation.~~
12. Add runtime `max_host_calls` and per-action budgets.
13. Expand protocol fixtures for remaining frame, envelope, signature, PoA, and
    control edge cases.
14. Broaden SDK conformance tests against fixtures beyond the current PACT,
    receipt, envelope, and control coverage.
15. Expand SDK surface to frame signing, verification, receipts, capabilities,
    PACT helpers, and agent messages.
16. ~~Add CLI workflows for agent intent/session/delegation/negotiation/status.~~
17. ~~Define the domain pack manifest format.~~
18. ~~Build `rivun-pack-agentic-dev` as the first complete domain pack.~~
19. ~~Build `rivun-pack-smart-building` or `rivun-pack-industrial` as the first
    real-world automation pack.~~
20. ~~Publish a release-readiness checklist tied to 1.0 gates.~~

Newly implemented beyond this list: `rivun-gateway` (MCP/HTTP/SSE/WebSocket +
provenance chain), BFT consensus and gossip/mesh in `rivun-net`, MMR/batch/ZK
receipt accumulation in `rivun-ledger`, async WASM pipeline and streaming buffers
in `rivun-runtime`, dispute/escrow engine in `rivun-pact`, pack build/sign/install
in `rivun-pack`, fleet doctor/incident snapshots in `rivun-telemetry`, and the
174-test 4-tier E2E suite in `tests/e2e`.

## Release Cadence

Until 1.0, releases should be small enough to audit and large enough to move a
visible adoption path forward.

Recommended cadence:

- weekly internal integration snapshots for contributors;
- monthly alpha releases while protocol and CLI surfaces are still moving;
- beta releases only after fixtures, docs, SDK conformance, and production
  checks cover the promoted surface;
- release candidates only after every official example passes
  `doctor --strict`, SDK conformance, pack validation, and website link checks.

Each release should publish:

- a short operator impact summary;
- compatibility notes for protocol, config, SDKs, drivers, and domain packs;
- migration steps for breaking changes;
- verification commands for signatures, checksums, fixtures, and examples;
- known limitations and explicit non-production warnings when applicable.

## Governance Guardrails

rivun can become broad without becoming vague if changes respect these guardrails:

- protocol changes require fixtures before implementation is considered done;
- new authority paths require policy, receipt, and observability coverage;
- new domain integrations start as packs or gateways, not core protocol forks;
- unsafe defaults must be documented as preview-only or removed before beta;
- convenience APIs cannot bypass identity, policy, PoA, grants, or receipts;
- examples must be runnable, validated, and owned by CI before they are treated
  as official adoption paths.

## Non-Goals

rivun should not become:

- a natural-language agent planner embedded in the protocol;
- a replacement for every message broker or RPC framework;
- a hidden database for model state;
- a financial ledger or cryptocurrency system;
- a runtime that grants ambient filesystem, network, clock, or environment
  access to untrusted drivers;
- an integration platform that weakens identity, policy, or receipt guarantees
  for convenience.

The core promise stays narrow and strong: rivun moves typed messages and actions
through a verifiable trust boundary. Everything else must reinforce that promise.

