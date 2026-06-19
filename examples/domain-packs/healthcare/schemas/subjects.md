# Healthcare Subjects

All healthcare subjects must include `schema_version = 1`, `patient_scope`,
`actor_id`, `reason`, `evidence_refs`, and `privacy_classification` fields.
Payloads must avoid embedding raw protected health information when a hashed or
redacted reference can prove the workflow.

| Subject | Kind | Risk | Required Evidence |
| --- | --- | --- | --- |
| `patient.record.read` | data | low | patient scope, purpose, grant id |
| `clinical.alert.route` | action | medium | alert id, care team route, urgency |
| `care.task.propose` | action | medium | proposed task, owner, due window |
| `record.note.write` | action | high | note hash, clinician approval, prior note ref |
| `device.command.propose` | action | high | device id, simulation ref, safety state |
| `clinical.order.place` | action | critical | order proposal, validator set, PoA certificate |
| `privacy.export` | action | critical | recipient, legal basis, export manifest, PoA |

Out of scope for the preview pack:

- autonomous diagnosis;
- medication execution without clinical authority;
- raw PHI broadcast subjects;
- device commands without simulation and explicit approval.
