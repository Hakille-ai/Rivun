# Deployment

ZAP can run directly from Cargo during development or from the production
container image in environments that prefer immutable artifacts.

## Build the Container

```bash
docker build -t zap:local .
```

The image is multi-stage:

- builder: Rust toolchain, compiles `zap-cli` in release mode;
- runtime: Debian slim, `tini`, non-root `zap` user, only the `zap` binary.

The default command is:

```bash
zap run --config /etc/zap/zap.toml
```

## Compose Template

Create a local state directory and node key:

```bash
mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key
```

Then start a node:

```bash
docker compose up --build
```

The default compose file mounts:

- `examples/configs/container-node.toml` to `/etc/zap/zap.toml`;
- `.zap/container` to `/var/lib/zap`;
- `examples/wasm-drivers` to `/opt/zap/drivers`.

The template binds the host UDP port to `127.0.0.1` by default. For production
networks, replace that host address with the intended interface and enforce
matching firewall policy.

For real deployments, copy the example config into your own private `.zap`
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
- writable state only under `/var/lib/zap`;
- loopback-only UDP port exposure by default.

Operators should additionally enforce host firewall rules and rotate transport
keys when peer membership changes.

## Production Checklist

- Generate one node key per deployment target.
- Store node private keys outside the repository.
- Configure static peers with verified `node_id`, `public_key`, and
  `transport_key`.
- Set `[peers.trust]` so send, receive, forwarding, PoA-attestation, expiry,
  and key-rotation policy are explicit for each machine.
- Set `[security]` replay and clock-skew limits intentionally.
- Use signed driver manifests for all production WASM drivers.
- Enable `[receipts].path` when audit trails are required.
- Run `zap capability cache refresh --config <path> --strict` before strict
  validation when routes require peer grants.
- Run `zap check-config --strict --config <path>` before starting a daemon.
- Pin container image digests in production orchestrators once images are
  published.
