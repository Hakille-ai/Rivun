# rivun Dynamic Discovery

rivun discovery lets an operator query configured seed peers for signed service,
capability, and peer inventory data without editing the local config for every
service change.

Discovery is intentionally layered on top of the existing encrypted rivun
transport. A node can exchange discovery messages with configured seed peers,
learn signed service announcements relayed by those peers, and optionally keep
those announcements on disk. It does not silently trust or activate unknown
transport keys.

## Control Subjects

Discovery uses signed `control` envelopes:

- `rivun.discovery.announce`: one peer sends a signed discovery advertisement.
- `rivun.discovery.query`: one peer asks for services, peer inventory, and known
  dynamic announcements.
- `rivun.discovery.response`: the queried peer returns its signed local
  advertisement, optional configured peer inventory, and optional announcements
  it has received.

All discovery envelopes use content type `application/rivun-discovery+json`.

## Trust Model

Each discovery advertisement is signed with the advertising node identity using
a discovery-specific Ed25519 domain. The receiving node also verifies the outer
rivun frame when `require_signed = true`.

This gives two checks:

- The transport frame proves which configured peer sent the message.
- The embedded advertisement remains independently verifiable if relayed,
  logged, or printed by the CLI.

Nodes keep received announcements in memory and can persist them with
`[discovery].announcement_cache = "discovery-announcements.jsonl"`. Operators
should still use the peer enrollment flow when a discovered node should become
a configured transport peer.

## CLI

Send a signed local announcement to a configured peer:

```powershell
rivun discovery announce --config rivun.toml --target <peer-node-id> --service echo=driver.execute:echo --json
```

If no `--service` is supplied, rivun derives services from the local capability
advertisement. A service spec can be either `id` or `id=capability`.

Query a configured peer:

```powershell
rivun discovery query --config rivun.toml --target <peer-node-id> --json
```

Useful filters:

```powershell
rivun discovery query --config rivun.toml --target <peer-node-id> --capability driver.execute:echo
rivun discovery query --config rivun.toml --target <peer-node-id> --service remote.status
rivun discovery query --config rivun.toml --target <peer-node-id> --no-peers --no-known
```

Enable durable announcement cache in node config:

```toml
[discovery]
announcement_cache = "state/discovery-announcements.jsonl"
```

The JSON response includes:

- `response.advertisement`: the queried peer's signed local advertisement.
- `response.peers`: sanitized configured peer inventory, excluding transport
  keys.
- `response.announcements`: signed dynamic advertisements received from other
  peers.

## Operator Notes

- Discovery respects peer trust. If `allow_send = false`, local queries and
  announcements to that target are rejected.
- Responses never expose `transport_key`.
- Dynamic announcements are durable when `discovery.announcement_cache` is set;
  expired announcements are ignored on reload.
- `advertised_addr` is informational. It does not override the transport
  address in config.
- Use `rivun peer invite` / `rivun peer accept` for durable peer enrollment, then
  use discovery to inspect changing services and capabilities.

