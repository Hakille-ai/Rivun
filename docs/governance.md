# Governance

ZAP production operations should require explicit human authority for changes
that alter executable code, trust roots, release artifacts, or emergency
controls. `zap-ops` provides typed contracts for groups, roles, multi-sig
approval policies, and hash-chained audit entries.

The reference policy is
`crates/zap-ops/config/governance/production-governance.toml`.

## Roles

Recommended roles:

- `release_manager`: cuts public releases and approves release notes;
- `security_officer`: reviews trust roots, signing state, and security notes;
- `registry_maintainer`: publishes, deprecates, or revokes ZapStore bundles;
- `sre`: operates production nodes;
- `incident_commander`: authorizes break-glass actions;
- `auditor`: reads approval and receipt trails but does not approve changes.

Operators can hold multiple roles, but production policies should avoid having
one person satisfy every group in a critical workflow.

## Multi-Sig Policies

Use separate groups for separate risk domains:

- stable release: two release approvals plus one security approval;
- registry publication or revocation: one registry approval plus one security
  approval;
- break-glass operations: one incident commander or SRE approval, followed by a
  mandatory audit entry.

Approval decisions should include:

- policy id;
- request id;
- operator id;
- group id;
- role used for the approval;
- timestamp in microseconds;
- approve or reject decision.

Any rejection should fail the request until a new request id is opened.

## Audit Trail

Audit entries are append-only JSONL records. Each entry stores its sequence,
the previous entry hash, and its own BLAKE3 hash over the canonical entry body.
Verify the chain before and after release or registry actions:

```bash
cargo test -p zap-ops audit_chain_detects_mutation
```

Production automation should write entries for:

- approval request opened;
- approval granted or rejected;
- signed registry mutation;
- release package built;
- checksums and signatures published;
- break-glass action executed;
- receipt journal archival or compaction.

Keep audit JSONL files separate from signed receipt journals. Receipts prove node
processing; governance audit proves human and CI authority.

## Break Glass

Break-glass approval is for restoring service or stopping unsafe behavior, not
for bypassing release or registry review. A break-glass request must record:

- incident id;
- time window;
- actor and approving incident commander;
- exact command or config change;
- rollback plan;
- receipt or log paths used to verify outcome.

After the incident, open a follow-up review and rotate any temporary trust
material.
