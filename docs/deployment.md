# Deployment

rivun can run directly from Cargo during development or from the production
container image in environments that prefer immutable artifacts.

## Build the Container

```bash
docker build -t rivun:local .
```

The image is multi-stage:

- builder: Rust toolchain, compiles `rivun-cli` in release mode;
- runtime: Debian slim, `tini`, non-root `rivun` user, only the `rivun` binary.

The default command is:

```bash
rivun run --config /etc/rivun/rivun.toml
```

## Compose Template

Create a local state directory and node key:

```bash
mkdir -p .rivun/container
docker compose run --rm node keygen --out /var/lib/rivun/node.key
```

Then start a node:

```bash
docker compose up --build
```

The default compose file mounts:

- `examples/configs/container-node.toml` to `/etc/rivun/rivun.toml`;
- `.rivun/container` to `/var/lib/rivun`;
- `examples/wasm-drivers` to `/opt/rivun/drivers`.

The template binds the host UDP port to `127.0.0.1` by default. For production
networks, replace that host address with the intended interface and enforce
matching firewall policy.

For real deployments, copy the example config into your own private `.rivun`
state directory, add peers and drivers, then update the compose volume to mount
that config instead.

## Runtime Hardening

The compose template uses:

- non-root container user;
- read-only root filesystem;
- dropped Linux capabilities;
- `no-new-privileges`;
- bounded process count;
- `noexec`, `nosuid`, and `nodev` temporary filesystem;
- writable state only under `/var/lib/rivun`;
- loopback-only UDP port exposure by default.

Operators should additionally enforce host firewall rules and rotate transport
keys when peer membership changes.

## Production Checklist

- Generate one node key per deployment target.
- Store node private keys outside the repository.
- Configure static peers with verified `node_id`, `public_key`, and
  `transport_key`.
- Prefer `rivun peer invite` and `rivun peer accept` for signed peer enrollment
  material, and use `rivun peer rotate` / `rivun peer revoke` for auditable
  membership changes.
- Set `[peers.trust]` so send, receive, forwarding, PoA-attestation, expiry,
  and key-rotation policy are explicit for each machine.
- Set `[security]` replay and clock-skew limits intentionally.
- Use signed driver manifests for all production WASM drivers.
- Use `rivun registry pull --operator-public-key <key>` when mirroring registry
  indexes from peers, and keep `[registry] require_signature = true` for
  deployment configs.
- Use `rivun registry mirror --operator-public-key <key>` to consolidate multiple
  peer indexes, then review and re-sign the merged registry before deployment.
- Create and archive `rivun registry publication create` output for every
  approved registry rollout so audits can verify the exact deployed index hash.
- Use `rivun registry bundle pull-manifest --require-publication --require-drivers`
  to inspect a peer-published bundle contract before fetching artifacts.
- Use `rivun registry bundle verify --require-drivers` before importing offline
  RivunStore bundles into production or factory images.
- Enable `[receipts].dir` when audit trails are required.
- Run `rivun capability cache refresh --config <path> --strict` before strict
  validation when routes require peer grants.
- Use `rivun poa validator-set verify` before deploying signed validator-set
  files referenced by `[poa].validator_set`.
- Use `rivun poa validator-set pull --authority-public-key <key>` when fetching
  validator sets from peers, then run strict config validation on the applied
  config.
- Run `rivun check-config --strict --config <path>` before starting a daemon.
- Run `rivun doctor --strict --config <path>` as a pre-flight readiness gate
  before placing the node into service.

## Rivun Cloud SaaS Deployment

Rivun Cloud services can be deployed alongside your edge fleets:

### 1. Rivun Cloud API (`crates/rivun-cloud-api`)
```bash
# Run standalone API daemon
cargo run --release -p rivun-cloud-api -- --host 0.0.0.0 --port 8080
```
Environment variables:
- `RIVUN_CLOUD_HOST`: Bind address (default `127.0.0.1`)
- `RIVUN_CLOUD_PORT`: HTTP and SSE port (default `8080`)
- `RUST_LOG`: Log filter (e.g. `info,rivun_cloud_api=debug`)

### 2. Rivun Dashboard (`apps/rivun-dashboard`)
```bash
cd apps/rivun-dashboard
npm install
npm run build
npm run start
```
Environment variables:
- `NEXT_PUBLIC_RIVUN_API_URL`: URL of the Rivun Cloud API backend (default `http://localhost:8080`).

### 3. Edge Bridge Daemon (`crates/rivun-cloud-bridge`)
Connect an edge node to Rivun Cloud:
```bash
# Bridge daemon connects node to SaaS
cargo run --release -p rivun-cloud-bridge -- \
  --cloud-url https://api.rivun.cloud \
  --org acme \
  --token <API_BEARER_TOKEN> \
  --node-id <NODE_UUID> \
  --node-label fra1-edge-01 \
  --active-policy-path /etc/rivun/policy.toml
```
- Pin container image digests in production orchestrators once images are
  published.
