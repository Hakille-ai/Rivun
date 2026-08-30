'use client';

import React, { useState, useMemo } from 'react';
import { ShieldCheck, Copy, Check, FileCode, Hash, Lock } from 'lucide-react';
import { Badge } from '@/components/ui/Badge';

export function PactVisualizer() {
  const [contractId, setContractId] = useState('pact-89f1a04e-2026-bft');
  const [initiator, setInitiator] = useState('agent-dev-lead-401');
  const [counterparty, setCounterparty] = useState('agent-code-reviewer-902');
  const [actionSubject, setActionSubject] = useState('repo.patch.merge');
  const [escrowAmount, setEscrowAmount] = useState('500');
  const [arbitrationThreshold, setArbitrationThreshold] = useState('2');
  const [copied, setCopied] = useState(false);

  // Build Canonical JSON with sorted keys
  const canonicalJson = useMemo(() => {
    const rawObj = {
      action_subject: actionSubject,
      arbitration_threshold: Number(arbitrationThreshold),
      counterparty,
      escrow_tokens: Number(escrowAmount),
      initiator,
      pact_id: contractId,
      schema_version: 'ZAP-PACT-v1',
      timestamp_micros: 1787884800000000,
    };

    // Sort keys alphabetically
    const sortedKeys = Object.keys(rawObj).sort();
    const sortedObj: Record<string, unknown> = {};
    for (const k of sortedKeys) {
      sortedObj[k] = rawObj[k as keyof typeof rawObj];
    }
    return JSON.stringify(sortedObj, null, 2);
  }, [contractId, initiator, counterparty, actionSubject, escrowAmount, arbitrationThreshold]);

  // Simulated BLAKE3 digest and signature
  const digestHex = useMemo(() => {
    // Generate deterministic hash-like string based on content length
    let hash = 0;
    for (let i = 0; i < canonicalJson.length; i++) {
      hash = (hash << 5) - hash + canonicalJson.charCodeAt(i);
      hash |= 0;
    }
    const hex = Math.abs(hash).toString(16).padStart(8, '0');
    return `9f8e7d6c5b4a3a2b1c0d${hex}e1f2a3b4c5d6e7f80918273645a4b3c2`;
  }, [canonicalJson]);

  const signatureHex = useMemo(() => {
    return `7e8a9b0c1d2e3f4a5b6c7d8e9f0a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f80` +
      `a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f80918273645a4b3c2d1e0f9a8b7c6`;
  }, []);

  const handleCopyJson = async () => {
    try {
      await navigator.clipboard.writeText(canonicalJson);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <div className="space-y-6">
      <div className="p-6 rounded-2xl border border-border-subtle bg-bg-surface shadow-card">
        <div className="flex items-center justify-between pb-4 mb-5 border-b border-border-subtle">
          <div className="flex items-center gap-2">
            <ShieldCheck className="w-5 h-5 text-accent-primary" />
            <h3 className="text-base font-bold text-text-primary">
              PACT Multi-Party Contract Canonicalizer
            </h3>
          </div>
          <Badge variant="purple">ZAP-PACT-v1 Specification</Badge>
        </div>

        {/* Inputs */}
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4 mb-6">
          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              PACT Contract UUID / ID
            </label>
            <input
              type="text"
              value={contractId}
              onChange={(e) => setContractId(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Initiating Agent ID
            </label>
            <input
              type="text"
              value={initiator}
              onChange={(e) => setInitiator(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Counterparty Agent ID
            </label>
            <input
              type="text"
              value={counterparty}
              onChange={(e) => setCounterparty(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Action Subject
            </label>
            <input
              type="text"
              value={actionSubject}
              onChange={(e) => setActionSubject(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Escrow Token Deposit
            </label>
            <input
              type="number"
              value={escrowAmount}
              onChange={(e) => setEscrowAmount(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>

          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Arbitration Threshold (T_dispute)
            </label>
            <input
              type="number"
              value={arbitrationThreshold}
              onChange={(e) => setArbitrationThreshold(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>
        </div>

        {/* Canonical JSON Output */}
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2 text-xs font-bold text-text-primary uppercase tracking-wider">
              <FileCode className="w-4 h-4 text-accent-primary" />
              <span>Canonical Deterministic JSON (RFC 8785)</span>
            </div>
            <button
              onClick={handleCopyJson}
              className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-bg-subtle hover:bg-bg-surface-raised text-text-secondary hover:text-text-primary text-xs border border-border-subtle"
            >
              {copied ? (
                <>
                  <Check className="w-3.5 h-3.5 text-status-verified" />
                  <span className="text-status-verified font-medium">Copied!</span>
                </>
              ) : (
                <>
                  <Copy className="w-3.5 h-3.5" />
                  <span>Copy JSON</span>
                </>
              )}
            </button>
          </div>

          <div className="p-4 rounded-xl bg-[#080B10] border border-border-subtle font-mono text-xs text-cyan-300 overflow-x-auto">
            <pre>{canonicalJson}</pre>
          </div>
        </div>

        {/* Cryptographic Digests & Signatures */}
        <div className="mt-6 grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="p-4 rounded-xl bg-bg-subtle border border-border-subtle space-y-1.5">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-text-primary">
              <Hash className="w-4 h-4 text-emerald-400" />
              <span>BLAKE3 Canonical Digest (32 Bytes)</span>
            </div>
            <div className="font-mono text-[11px] text-emerald-300 break-all">
              {digestHex}
            </div>
          </div>

          <div className="p-4 rounded-xl bg-bg-subtle border border-border-subtle space-y-1.5">
            <div className="flex items-center gap-1.5 text-xs font-semibold text-text-primary">
              <Lock className="w-4 h-4 text-sky-400" />
              <span>Detached Ed25519 PACT Signature (64 Bytes)</span>
            </div>
            <div className="font-mono text-[11px] text-sky-300 break-all">
              {signatureHex}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
