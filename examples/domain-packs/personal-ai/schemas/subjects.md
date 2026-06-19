# Personal AI Subject Catalog

This pack defines auditable subjects for personal AI assistants that operate across private accounts, local context, and third-party services.

All actions are fail-closed. A subject must be declared in `pack.toml`, covered by `policies/action-policy.toml`, and bounded by a user-visible grant, approval, proof of authority, or deny rule.

## Low-risk reads

- `calendar.read`: read bounded calendar availability, event metadata, reminders, or invite context.
- `email.read`: read bounded mailbox, thread, label, search result, header, snippet, or message context.
- `memory.read`: read local memory entries, preferences, task state, or assistant context from an approved namespace.

Expected bounds:
- account or namespace;
- subject, thread, event, path, query, or time window;
- purpose string;
- maximum result count or byte limit;
- retention and redaction expectations when personal data is present.

## Medium-risk draft and local state actions

- `email.draft`: create or update an email draft without sending.
- `calendar.propose`: prepare event drafts, meeting suggestions, agenda blocks, or reschedule proposals without committing changes.
- `file.write`: create or update bounded local files, notes, exports, and assistant artifacts.
- `memory.write`: store bounded personal preferences, durable task state, and local assistant memory.

Expected bounds:
- target account, path, namespace, or draft identifier;
- previewable before/after summary;
- overwrite behavior;
- retention policy;
- rollback or delete plan when applicable.

## High-risk external action gates

- `email.send`: send, reply, forward, or otherwise transmit email to recipients.
- `file.share`: share, publish, upload, or expose files outside the private workspace.
- `browser.automate`: click, submit, navigate authenticated sessions, fill forms, or trigger third-party workflows.
- `commerce.propose`: prepare carts, quotes, booking options, subscription candidates, or payment proposals without execution.

Required approval context:
- exact recipients, destinations, domains, or services;
- user-visible content summary;
- irreversible side effects;
- timeout and one-shot execution scope;
- evidence link or transcript for audit.

## Critical actions

- `money.execute`: execute payments, purchases, transfers, bookings, bids, subscriptions, trades, or other monetary commitments.
- `credential.use`: use, reveal, rotate, export, delegate, or store credentials, passwords, tokens, passkeys, keys, or recovery material.
- `data.destroy`: delete, purge, revoke, overwrite, or irreversibly destroy personal data, files, accounts, or memory.

Critical actions require proof of authority or are denied by policy. `credential.use` is denied by default in this pack because personal assistants should not handle secrets through ordinary action delegation.

## Payload Shape

Recommended action payloads include:

- `subject`: one of the subjects listed above;
- `actor`: assistant or driver identity;
- `user_scope`: account, namespace, workspace, or service boundary;
- `resource`: event, thread, file, memory key, browser origin, or commerce target;
- `intent`: concise user-visible reason;
- `bounds`: time window, count limit, path allowlist, recipient list, amount ceiling, or allowed domain list;
- `preview`: human-readable summary before external or destructive effects;
- `approval`: grant, human approval, proof of authority, or denial evidence;
- `audit`: correlation id, transcript link, policy version, and timestamp.
