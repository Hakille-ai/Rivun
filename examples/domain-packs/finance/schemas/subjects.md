# Finance Subjects

All finance subjects must include `schema_version = 1`, `account_scope`,
`actor_id`, `policy_context`, `risk_refs`, `evidence_refs`, and `idempotency_key`.
Execution subjects must also include approval references and a deterministic
proposal hash.

| Subject | Kind | Risk | Required Evidence |
| --- | --- | --- | --- |
| `account.read` | data | low | account scope, grant id, purpose |
| `risk.check` | action | medium | policy id, inputs hash, result hash |
| `trade.propose` | action | medium | proposal hash, risk check ref |
| `payment.propose` | action | medium | proposal hash, risk check ref |
| `approval.record` | action | high | approver id, approval scope, expiration |
| `trade.execute` | action | critical | proposal hash, approval ref, PoA certificate |
| `payment.execute` | action | critical | proposal hash, approval ref, PoA certificate |
| `reconciliation.finalize` | action | critical | reconciliation manifest, ledger refs, PoA |

Out of scope for the preview pack:

- ambient trading authority;
- hidden model-only approvals;
- irreversible execution without deterministic proposal hash;
- private key custody or direct wallet signing.
