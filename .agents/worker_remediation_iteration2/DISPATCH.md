## 2026-08-30T21:46:54Z

You are the Remediation Builder for the Rivun project.
Working directory: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\worker_remediation_iteration2
Project root: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun
Original request path: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\ORIGINAL_REQUEST.md
Challenger 1 findings & fix instructions: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\challenger_1_wire_and_consensus\handoff.md
Challenger 2 findings: c:\Users\Stagiaire\Documents\Amadou PGC\Prs\Rivun\.agents\challenger_2_docs_and_search\handoff.md

MANDATORY INTEGRITY WARNING:
DO NOT CHEAT. All implementations must be genuine. DO NOT hardcode test results, create dummy/facade implementations, or circumvent the intended task. A teamwork_preview_auditor will independently verify your work. Integrity violations WILL be detected and your work WILL be rejected.

Your mission:
1. Read the Challenger 1 and Challenger 2 handoff reports.
2. Apply the exact remediations identified by Challenger 1 and Challenger 2:
   a. In `apps/marketing-site/lib/protocol.ts`:
      - Fix payload length writing at lines 168-169: write 64-bit big-endian integer at offset 48 (`wireView.setBigUint64(48, BigInt(wirePayloadLen), false)`).
      - Fix ZENV 74-byte header encoding:
        `zenvView.setUint16(6, kindInfo.id, false);`
        `zenvView.setUint16(8, 0, false);` (reserved)
        `zenvBuffer.set(envUuidBytes, 10);`
        `zenvBuffer.set(corrBytes, 26);`
        `zenvBuffer.set(causBytes, 42);`
        `zenvView.setUint16(58, subjectBytes.length, false);`
        `zenvView.setUint16(60, contentTypeBytes.length, false);`
        `zenvView.setUint32(62, metadataBytes.length, false);`
        `zenvView.setBigUint64(66, BigInt(bodyBytes.length), false);`
      - Ensure `apps/marketing-site/components/HeroFrameVisualizer.tsx` and byte offset definitions in `protocol.ts` match the exact canonical byte offsets.
   b. In `tests/e2e/harness/zenvCodec.mjs`:
      - In `RivunEnvelope.decode(buf)`, add check that bytes 8..10 (`reserved`) === 0.
      - Add check that non-Data kinds require a non-empty `subject`.
   c. In `apps/docs-portal/public/search-index.json`:
      - Sync all 77 document records from `generateSearchIndex()` so static consumers have the full index.
3. Build & Test Verification:
   - Run `npm run build` in `apps/marketing-site` (must pass with 0 errors).
   - Run `npm run build` and `npm run typecheck` in `apps/docs-portal` (must pass with 0 errors).
   - Run `node tests/e2e/test-runner.mjs` (must pass with exit code 0).
   - Run `node tests/e2e/challenger1_empirical_stress.mjs` and `node tests/docs_portal_empirical_stress_runner.mjs` if available to ensure all stress tests pass.
4. Write your self-contained `handoff.md` with build logs and test outputs in your working directory and notify the parent orchestrator.
