# Security Policy

rivun is security-sensitive infrastructure. Please report suspected
vulnerabilities privately before publishing details.

## Supported Versions

| Version | Security support |
| --- | --- |
| `main` | Best-effort fixes before the next release |
| `0.1.x` | Supported while the project is pre-1.0 |

Security fixes may be released as patch versions without waiting for a feature
release.

## Reporting a Vulnerability

Preferred path:

1. Use GitHub private vulnerability reporting if it is enabled for the
   repository.
2. If private reporting is not available yet, request a private maintainer
   contact channel through the repository owner profile and include
   `[SECURITY]` in the subject.

Please include:

- affected commit, tag, or crate version;
- operating system and architecture;
- reproduction steps or a minimal proof of concept;
- expected impact, including whether private keys, signatures, transport
  confidentiality, replay protection, WASM isolation, or PoA verification are
  involved;
- whether the issue is already public.

Do not include production private keys, transport keys, or secrets in a report.

## Disclosure Process

The maintainers aim to acknowledge valid reports within 7 days. The expected
flow is:

1. confirm scope and severity;
2. prepare a fix and regression test;
3. publish a security advisory or release notes;
4. credit the reporter when requested.

For critical issues with active exploitation, maintainers may accelerate
disclosure and release.

## Security Boundaries

The following are in scope:

- malformed @@@@rivun_HEADER@@WIRE@@ or `ZENV` data causing panic, memory pressure, or parser
  bypass;
- signature, PoA, replay, or key identity bypass;
- plaintext exposure in encrypted datagrams;
- WASM sandbox escape or missing permission enforcement;
- unsafe defaults in daemon, CLI, Docker, or examples.

The following are out of scope unless they demonstrate a rivun bug:

- attacks requiring access to a node private key;
- denial of service from unlimited external traffic on an intentionally exposed
  UDP port;
- issues in unreleased roadmap features.

