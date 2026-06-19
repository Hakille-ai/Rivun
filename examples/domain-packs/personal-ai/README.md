# Personal AI Domain Pack

The Personal AI pack models a serious assistant that can help with calendar, email, files, browser sessions, local memory, and proposal-only commerce while keeping private-user authority explicit and auditable.

The default posture is fail-closed:

- low-risk reads require explicit grants;
- medium-risk draft, file, and memory writes require scoped grants;
- high-risk email send, file sharing, browser automation, and commerce proposals require human approval;
- critical money, destructive, and credential actions require proof of authority or are denied.

## Capabilities

| Capability | Risk | Gate |
| --- | --- | --- |
| `calendar.read` | low | Explicit grant |
| `email.read` | low | Explicit grant |
| `memory.read` | low | Explicit grant |
| `email.draft` | medium | Explicit grant |
| `calendar.propose` | medium | Explicit grant |
| `file.write` | medium | Explicit grant |
| `memory.write` | medium | Explicit grant |
| `email.send` | high | Human approval |
| `file.share` | high | Human approval |
| `browser.automate` | high | Human approval |
| `commerce.propose` | high | Human approval, proposal-only |
| `money.execute` | critical | Proof of authority |
| `credential.use` | critical | Denied |
| `data.destroy` | critical | Proof of authority |

## Example Flows

### Draft an email

1. Grant `email.read` for the selected thread.
2. Grant `email.draft` for a specific mailbox and draft target.
3. Review the generated draft before any send action.

### Schedule a meeting proposal

1. Grant `calendar.read` for the relevant time window.
2. Use `calendar.propose` to prepare candidate times or a draft invite.
3. Keep the action proposal-only until the user approves a concrete calendar change.

### Prepare a purchase proposal

1. Use `browser.automate` only with human approval for the selected site and session.
2. Use `commerce.propose` to prepare options, carts, quotes, or booking candidates.
3. Require `money.execute` with proof of authority before any payment, bid, subscription, or purchase.

## Validate

```powershell
cargo run -p zap-cli -- pack validate --pack examples/domain-packs/personal-ai --json
```
