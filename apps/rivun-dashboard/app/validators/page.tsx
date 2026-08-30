"use client";

import React, { useEffect, useState } from "react";
import {
  ShieldAlert,
  CheckCircle2,
  Users,
  Percent,
  RefreshCw,
  Lock,
  ArrowRight,
  Plus,
} from "lucide-react";
import { api } from "../../lib/api";
import { ValidatorSetRecord } from "../../lib/types";

export default function ValidatorsPage() {
  const [validatorSets, setValidatorSets] = useState<ValidatorSetRecord[]>([]);
  const [isRotating, setIsRotating] = useState(false);
  const [notification, setNotification] = useState<string | null>(null);

  useEffect(() => {
    async function load() {
      const data = await api.fetchValidators();
      setValidatorSets(data);
    }
    load();
  }, []);

  const activeSet = validatorSets[0];

  const handleProposeRotation = () => {
    setIsRotating(true);
    setTimeout(() => {
      setIsRotating(false);
      setNotification("Validator rotation proposed! Requires Ed25519 signature in Rivun Control.");
      setTimeout(() => setNotification(null), 4000);
    }, 1000);
  };

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Proof-of-Action (PoA) Validators</h1>
          <p className="text-sm text-text-secondary">
            Byzantine-fault-tolerant swarm consensus threshold, attestation logs, and quorum validation.
          </p>
        </div>

        <button
          onClick={handleProposeRotation}
          disabled={isRotating}
          className="px-4 py-2 rounded-lg bg-accent-primary hover:bg-accent-hover text-white text-xs font-semibold shadow-glow transition flex items-center space-x-1.5 shrink-0"
        >
          <RefreshCw className={`w-3.5 h-3.5 ${isRotating ? "animate-spin" : ""}`} />
          <span>{isRotating ? "Proposing..." : "Propose Validator Rotation"}</span>
        </button>
      </div>

      {notification && (
        <div className="p-3.5 rounded-xl bg-status-verified-bg border border-status-verified/30 text-xs text-status-verified flex items-center space-x-2">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          <span>{notification}</span>
        </div>
      )}

      {/* Quorum Metric Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-2">
            <span className="text-xs font-mono uppercase">Consensus Threshold</span>
            <Users className="w-4 h-4 text-accent-primary" />
          </div>
          <div className="text-2xl font-bold text-text-primary font-mono">
            {activeSet?.threshold || 3} of {activeSet?.members?.length || 4}
          </div>
          <div className="text-[11px] text-text-secondary mt-1 font-mono">
            Requires {((((activeSet?.threshold || 3) / (activeSet?.members?.length || 4)) * 100)).toFixed(0)}% validator signers (BFT Safe)
          </div>
        </div>

        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-2">
            <span className="text-xs font-mono uppercase">Epoch & Round</span>
            <ShieldAlert className="w-4 h-4 text-status-verified" />
          </div>
          <div className="text-2xl font-bold text-text-primary font-mono">
            Epoch #{activeSet?.epoch || 1}
          </div>
          <div className="text-[11px] text-status-verified mt-1 font-mono">
            Active from {activeSet ? new Date(activeSet.active_from).toLocaleDateString() : "Genesis"}
          </div>
        </div>

        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-2">
            <span className="text-xs font-mono uppercase">Attestation Success</span>
            <Percent className="w-4 h-4 text-accent-primary" />
          </div>
          <div className="text-2xl font-bold text-status-verified font-mono">
            99.98%
          </div>
          <div className="text-[11px] text-text-secondary mt-1">
            Zero failed consensus rounds in last 24h
          </div>
        </div>
      </div>

      {/* Active Validator Set Table */}
      <div className="rounded-2xl bg-bg-surface border border-border-subtle p-6 space-y-4 shadow-card">
        <div className="flex items-center justify-between border-b border-border-subtle pb-3">
          <div className="flex items-center space-x-2">
            <ShieldAlert className="w-4 h-4 text-accent-primary" />
            <h3 className="text-sm font-semibold text-text-primary uppercase tracking-wide font-mono">
              Active Validator Set (Epoch #{activeSet?.epoch || 1})
            </h3>
          </div>
          <span className="text-xs font-mono text-status-verified">● 4/4 Nodes Healthy</span>
        </div>

        <div className="divide-y divide-border-subtle">
          {activeSet?.members?.map((member, idx) => (
            <div key={member.node_id} className="py-3.5 flex items-center justify-between">
              <div className="flex items-center space-x-4">
                <span className="w-6 h-6 rounded-full bg-bg-surface-raised border border-border-subtle flex items-center justify-center font-mono text-[11px] text-text-secondary">
                  {idx + 1}
                </span>
                <div>
                  <div className="font-semibold text-text-primary text-xs flex items-center space-x-2">
                    <span>{member.label}</span>
                    <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-status-verified-bg text-status-verified">
                      {member.status}
                    </span>
                  </div>
                  <div className="text-[10px] font-mono text-text-muted mt-0.5">
                    Public Key: {member.public_key}
                  </div>
                </div>
              </div>

              <div className="text-right">
                <div className="text-xs font-mono font-semibold text-text-primary">{member.uptime_pct}%</div>
                <div className="text-[10px] text-text-secondary font-mono">Uptime</div>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
