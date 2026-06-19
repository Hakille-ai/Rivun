# Agentic Development Subjects

This preview catalog documents the action subjects used by the agentic
development pack. Concrete JSON schemas will be added once domain-pack
validation is implemented.

## `repo.read`

Read-only repository inspection.

Expected payload fields:

- `path`: repository-relative path or glob;
- `reason`: human-readable reason for the read;
- `max_bytes`: optional read bound.

## `repo.patch`

Prepare or apply a scoped source patch.

Expected payload fields:

- `patch`: unified diff or patch artifact reference;
- `scope`: repository-relative paths the patch may touch;
- `intent_id`: linked agent intent id.

## `test.run`

Run bounded tests, checks, or linters.

Expected payload fields:

- `command`: allowlisted test command id;
- `timeout_ms`: execution timeout;
- `scope`: package, crate, module, or path being tested.

## `ci.inspect`

Inspect CI checks and logs.

Expected payload fields:

- `provider`: CI provider or gateway id;
- `run_id`: CI run identifier;
- `include_logs`: whether bounded logs may be returned.

## `pr.create`

Create or update a pull request.

Expected payload fields:

- `title`: pull request title;
- `body`: pull request body or artifact reference;
- `branch`: source branch;
- `base`: target branch;
- `evidence`: receipt, test, and review references.
