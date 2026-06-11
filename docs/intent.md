# Intent Compiler

`zap-intent` is the local Phase 2 foundation from the PDF roadmap. It converts a human or agent intent into an auditable action plan before any frame is sent. This is an adapter above the protocol, not a requirement of the protocol itself.

The compiler is deterministic in v1. It does not call a remote LLM, does not depend on any AI model provider, and does not hide decisions. The output is a JSON plan with one or more steps:

```bash
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20 et declencher arret urgence robot"
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20" --explain
```

Each step contains:

- `action`: driver action name;
- `payload`: text or compact JSON payload;
- `payload_format`: `text` or `json`;
- `requires_consensus`: whether the frame should set `REQUIRES_CONSENSUS`;
- `rationale`: why the rule emitted the step.

`zap send --intent` compiles and sends every step to the target:

```bash
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "Ajuster la temperature a 20"
```

Intent policies are optional JSON files evaluated before frames are emitted. A
rule can `allow`, `deny`, or `require_poa` for matching `kind`, `subject`, or
`action` fields. Omitted match fields are wildcards.

```json
{
  "rules": [
    {
      "subject": "thermostat.setpoint",
      "decision": "require_poa",
      "reason": "temperature changes require operator approval"
    }
  ]
}
```

Use the same policy with plan inspection or send:

```bash
cargo run -p zap-cli -- compile-intent "Ajuster la temperature a 20" --policy policy.json --explain
cargo run -p zap-cli -- send --config zap.toml --target <uuid> --intent "Ajuster la temperature a 20" --policy policy.json
```

Intent steps map naturally to universal envelopes with `kind=action` and `subject=<action name>`. `zap send --intent` emits `ZENV` envelopes; `zap-node` still accepts the older JSON action shape for compatibility.

Supported v1 rules:

- `echo ...` emits `echo`;
- thermostat/temperature intents emit `thermostat.setpoint` with `{"temperature_c": ...}`;
- emergency-stop safety intents emit `safety.emergency_stop` and set `REQUIRES_CONSENSUS`;
- structured JSON intents with `action`, optional `payload`, and optional `requires_consensus` pass through deterministically.

Unsupported intents are rejected instead of guessed. This keeps the system safe while the future cognitive layer evolves.
