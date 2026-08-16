# ZAP Website

Marketing and documentation site for ZAP, built with Next.js (App Router) and
Tailwind CSS.

## Development

```bash
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000).

## Checks

```bash
npm run lint          # ESLint
npm run build         # production build (typecheck + static generation)
```

The `website lint` job in `.github/workflows/ci.yml` runs lint and build for
every push; benchmark results are published to
[hakille-ai.github.io/ZAP](https://hakille-ai.github.io/ZAP) by the
`performance` workflow (see `tools/xtask`).

## Layout

- `app/` — routes: home (`/`), vision (`/vision`), and docs (`/docs/*`)
- `components/` — shared UI components
- Docs pages mirror the repository's `docs/` content; keep both in sync when
  editing protocol or security material.

Source: [github.com/Hakille-ai/ZAP](https://github.com/Hakille-ai/ZAP).
