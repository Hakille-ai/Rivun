"use client";

import React, { useState } from "react";
import { X, CheckCircle2, ShieldCheck, Copy, Check } from "lucide-react";

interface Props {
  isOpen: boolean;
  receiptHash: string;
  onClose: () => void;
}

export function OfflineVerifierModal({ isOpen, receiptHash, onClose }: Props) {
  const [copied, setCopied] = useState(false);

  if (!isOpen) return null;

  const cliCommand = `rivun receipts verify --hash ${receiptHash} --offline`;

  const copyCommand = () => {
    navigator.clipboard.writeText(cliCommand);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-in fade-in duration-200">
      <div className="w-full max-w-xl rounded-2xl bg-bg-surface-raised border border-border-strong p-6 shadow-modal space-y-6 relative">
        <button
          onClick={onClose}
          className="absolute top-4 right-4 p-2 rounded-lg text-text-secondary hover:text-text-primary hover:bg-bg-surface transition"
        >
          <X className="w-4 h-4" />
        </button>

        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-xl bg-status-verified-bg text-status-verified flex items-center justify-center border border-status-verified/30">
            <ShieldCheck className="w-6 h-6" />
          </div>
          <div>
            <h3 className="text-base font-semibold text-text-primary">Offline Cryptographic Verifier</h3>
            <p className="text-xs text-text-secondary">Independent verification using BLAKE3 & Ed25519</p>
          </div>
        </div>

        {/* Verification explanation */}
        <div className="space-y-3 text-xs text-text-secondary leading-relaxed bg-bg-base p-4 rounded-xl border border-border-subtle">
          <div className="font-semibold text-text-primary flex items-center space-x-2">
            <CheckCircle2 className="w-4 h-4 text-status-verified" />
            <span>Mathematical Proof Explanation</span>
          </div>
          <p>
            Unlike traditional cloud broker platforms where you must trust the SaaS database, every ZAP receipt is
            reconstructed mathematically from the hash chain:
          </p>
          <div className="p-2.5 rounded bg-bg-surface font-mono text-[11px] text-accent-primary border border-border-subtle">
            H_intent &rarr; H_negotiation &rarr; H_policy &rarr; H_consensus &rarr; H_driver &rarr; H_poa &rarr; H_receipt &rarr; H_root
          </div>
          <p>
            You can verify this receipt on an air-gapped machine without network access using the open-source CLI.
          </p>
        </div>

        {/* CLI command snippet */}
        <div className="space-y-2">
          <div className="text-xs font-medium text-text-primary">CLI Verification Command</div>
          <div className="flex items-center justify-between p-3 rounded-lg bg-bg-base border border-border-subtle font-mono text-xs text-text-primary">
            <span className="truncate mr-2">{cliCommand}</span>
            <button
              onClick={copyCommand}
              className="px-2.5 py-1 rounded bg-bg-surface border border-border-subtle text-text-secondary hover:text-text-primary transition flex items-center space-x-1.5 shrink-0"
            >
              {copied ? <Check className="w-3.5 h-3.5 text-status-verified" /> : <Copy className="w-3.5 h-3.5" />}
              <span>{copied ? "Copied" : "Copy"}</span>
            </button>
          </div>
        </div>

        <div className="flex justify-end pt-2">
          <button
            onClick={onClose}
            className="px-5 py-2 rounded-lg bg-accent-primary text-white text-xs font-semibold hover:bg-accent-hover transition shadow-glow"
          >
            Done
          </button>
        </div>
      </div>
    </div>
  );
}
