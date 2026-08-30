"use client";

import React, { useState } from "react";
import { ProvenanceChain, ProvenanceStep } from "../lib/types";
import { ShieldCheck, ArrowRight, CheckCircle2, Hash, Layers, Key } from "lucide-react";

interface Props {
  provenance: ProvenanceChain;
  onVerifyOffline?: () => void;
}

export function ProvenanceGraph({ provenance, onVerifyOffline }: Props) {
  const [selectedStep, setSelectedStep] = useState<ProvenanceStep | null>(
    provenance.steps[0] || null
  );

  const stageColors: Record<string, { bg: string; text: string; border: string }> = {
    intent: { bg: "bg-blue-500/10", text: "text-blue-400", border: "border-blue-500/30" },
    negotiation: { bg: "bg-purple-500/10", text: "text-purple-400", border: "border-purple-500/30" },
    policy: { bg: "bg-amber-500/10", text: "text-amber-400", border: "border-amber-500/30" },
    consensus: { bg: "bg-indigo-500/10", text: "text-indigo-400", border: "border-indigo-500/30" },
    driver: { bg: "bg-cyan-500/10", text: "text-cyan-400", border: "border-cyan-500/30" },
    poa: { bg: "bg-emerald-500/10", text: "text-emerald-400", border: "border-emerald-500/30" },
    receipt: { bg: "bg-emerald-500/20", text: "text-status-verified", border: "border-status-verified/40" },
  };

  return (
    <div className="space-y-6">
      {/* Header with Merkle root and verify button */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 p-4 rounded-xl bg-bg-surface-raised border border-border-subtle">
        <div className="flex items-center space-x-3">
          <div className="w-10 h-10 rounded-lg bg-status-verified-bg text-status-verified flex items-center justify-center border border-status-verified/30">
            <ShieldCheck className="w-5 h-5" />
          </div>
          <div>
            <div className="text-xs text-text-secondary">Merkle Provenance Root Digest</div>
            <div className="font-mono text-sm text-text-primary font-medium break-all">
              {provenance.root_hash}
            </div>
          </div>
        </div>

        {onVerifyOffline && (
          <button
            onClick={onVerifyOffline}
            className="px-4 py-2 rounded-lg bg-accent-primary text-white text-xs font-semibold hover:bg-accent-hover transition shadow-glow shrink-0 flex items-center space-x-2"
          >
            <Key className="w-3.5 h-3.5" />
            <span>Verify Offline (BLAKE3/Ed25519)</span>
          </button>
        )}
      </div>

      {/* Horizontal Interactive Timeline */}
      <div className="overflow-x-auto pb-4 pt-2">
        <div className="flex items-center space-x-3 min-w-max px-1">
          {provenance.steps.map((step, idx) => {
            const isSelected = selectedStep?.stage === step.stage;
            const style = stageColors[step.stage] || {
              bg: "bg-gray-500/10",
              text: "text-gray-400",
              border: "border-gray-500/30",
            };

            return (
              <React.Fragment key={step.stage}>
                <button
                  onClick={() => setSelectedStep(step)}
                  className={`p-3.5 rounded-xl border text-left transition-all relative ${
                    isSelected
                      ? `${style.bg} ${style.border} ring-2 ring-accent-primary/40 shadow-glow`
                      : "bg-bg-surface border-border-subtle hover:border-border-strong hover:bg-bg-surface-raised"
                  }`}
                >
                  <div className="flex items-center justify-between space-x-4 mb-2">
                    <span className={`text-xs font-bold uppercase tracking-wider font-mono ${style.text}`}>
                      #{idx + 1} {step.stage}
                    </span>
                    <CheckCircle2 className="w-4 h-4 text-status-verified" />
                  </div>
                  <div className="font-mono text-[11px] text-text-secondary">
                    {step.step_hash.slice(0, 14)}...
                  </div>
                </button>

                {idx < provenance.steps.length - 1 && (
                  <div className="text-text-muted flex items-center shrink-0 px-1">
                    <ArrowRight className="w-4 h-4 text-border-highlight" />
                  </div>
                )}
              </React.Fragment>
            );
          })}
        </div>
      </div>

      {/* Step Detailed Inspector */}
      {selectedStep && (
        <div className="p-5 rounded-xl bg-bg-surface border border-border-subtle space-y-4">
          <div className="flex items-center justify-between border-b border-border-subtle pb-3">
            <div className="flex items-center space-x-2">
              <Layers className="w-4 h-4 text-accent-primary" />
              <h4 className="text-sm font-semibold text-text-primary uppercase tracking-wide">
                Stage Inspector: <span className="text-accent-primary font-mono">{selectedStep.stage}</span>
              </h4>
            </div>
            <span className="text-xs font-mono text-status-verified flex items-center space-x-1">
              <CheckCircle2 className="w-3.5 h-3.5" />
              <span>Causal Hash Link Valid</span>
            </span>
          </div>

          <div className="grid grid-cols-1 md:grid-cols-2 gap-4 text-xs">
            <div className="space-y-1">
              <div className="text-text-secondary">Current Step Transition Digest</div>
              <div className="font-mono p-2.5 rounded-lg bg-bg-surface-raised border border-border-subtle text-text-primary break-all">
                {selectedStep.step_hash}
              </div>
            </div>

            <div className="space-y-1">
              <div className="text-text-secondary">Previous Chained Hash</div>
              <div className="font-mono p-2.5 rounded-lg bg-bg-surface-raised border border-border-subtle text-text-primary break-all">
                {selectedStep.previous_hash || "Genesis Intent (None)"}
              </div>
            </div>
          </div>

          {selectedStep.metadata && Object.keys(selectedStep.metadata).length > 0 && (
            <div className="space-y-1.5 pt-2">
              <div className="text-xs text-text-secondary">Authenticated Stage Metadata</div>
              <pre className="font-mono text-[11px] p-3 rounded-lg bg-bg-base border border-border-subtle text-text-secondary overflow-x-auto">
                {JSON.stringify(selectedStep.metadata, null, 2)}
              </pre>
            </div>
          )}
        </div>
      )}
    </div>
  );
}
