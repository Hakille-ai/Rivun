"use client";

import React, { useEffect, useState } from "react";
import {
  Scale,
  Plus,
  Lock,
  CheckCircle2,
  AlertTriangle,
  ArrowRight,
  Code,
  Sliders,
  Check,
  Shield,
  Trash2,
} from "lucide-react";
import { api } from "../../lib/api";
import { PolicyRecord } from "../../lib/types";

interface VisualRule {
  id: string;
  name: string;
  kind?: string;
  subject?: string;
  decision: "allow" | "deny" | "require_poa" | "require_grant";
  required_capability?: string;
}

export default function PoliciesPage() {
  const [policies, setPolicies] = useState<PolicyRecord[]>([]);
  const [selectedPolicy, setSelectedPolicy] = useState<PolicyRecord | null>(null);
  const [activeTab, setActiveTab] = useState<"visual" | "diff" | "toml">("visual");

  // New Rule Builder State
  const [rules, setRules] = useState<VisualRule[]>([
    { id: "1", name: "allow_telemetry", kind: "telemetry", decision: "allow" },
    { id: "2", name: "enforce_consensus_safety", subject: "safety.*", decision: "require_poa" },
    { id: "3", name: "allow_smart_building", subject: "smart_building.*", decision: "require_poa" },
    { id: "4", name: "grant_driver_echo", subject: "driver.echo.*", decision: "require_grant", required_capability: "driver.execute:echo" },
  ]);

  const [policyName, setPolicyName] = useState("production-zero-trust-v3");
  const [isStaging, setIsStaging] = useState(false);
  const [isSigning, setIsSigning] = useState(false);
  const [notification, setNotification] = useState<string | null>(null);

  useEffect(() => {
    async function load() {
      const data = await api.fetchPolicies();
      setPolicies(data);
      if (data.length > 0) {
        setSelectedPolicy(data.find((p) => p.status === "staged") || data[0]);
      }
    }
    load();
  }, []);

  const handleAddRule = () => {
    setRules((prev) => [
      ...prev,
      {
        id: Date.now().toString(),
        name: `custom_rule_${prev.length + 1}`,
        subject: "agent.*",
        decision: "require_poa",
      },
    ]);
  };

  const handleRemoveRule = (id: string) => {
    setRules((prev) => prev.filter((r) => r.id !== id));
  };

  const handleStageCurrent = async () => {
    setIsStaging(true);
    // Generate TOML from visual rules
    const tomlLines = ['default_decision = "deny"', ""];
    rules.forEach((r) => {
      tomlLines.push("[[rules]]");
      tomlLines.push(`name = "${r.name}"`);
      if (r.kind) tomlLines.push(`kind = "${r.kind}"`);
      if (r.subject) tomlLines.push(`subject = "${r.subject}"`);
      tomlLines.push(`decision = "${r.decision}"`);
      if (r.required_capability) tomlLines.push(`required_capability = "${r.required_capability}"`);
      tomlLines.push("");
    });
    const toml = tomlLines.join("\n");

    const created = await api.createPolicy(policyName, toml);
    await api.stagePolicy(created.id);
    const updated = await api.fetchPolicies();
    setPolicies(updated);
    setSelectedPolicy(created);
    setIsStaging(false);
    setNotification("Policy bundle staged! Awaiting operator Ed25519 signature.");
    setTimeout(() => setNotification(null), 4000);
  };

  const handleSimulateLocalSign = async () => {
    if (!selectedPolicy) return;
    setIsSigning(true);
    await api.signPolicy(
      selectedPolicy.id,
      "MC4CAQAwBQYDK2VwBCIEIKU3L5Q2U9...",
      "4a8b9cdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
    );
    const updated = await api.fetchPolicies();
    setPolicies(updated);
    setSelectedPolicy(updated.find((p) => p.id === selectedPolicy.id) || null);
    setIsSigning(false);
    setNotification("Cryptographic signature submitted to Cloud! Edge nodes will pull on next poll.");
    setTimeout(() => setNotification(null), 4000);
  };

  const activePolicy = policies.find((p) => p.status === "active");

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Policy Studio & Staging</h1>
          <p className="text-sm text-text-secondary">
            Deterministic security policy editor with cryptographic human-in-the-loop signing workflow.
          </p>
        </div>

        <div className="flex items-center space-x-3">
          <button
            onClick={handleStageCurrent}
            disabled={isStaging}
            className="px-4 py-2 rounded-lg bg-accent-primary hover:bg-accent-hover text-white text-xs font-semibold shadow-glow transition flex items-center space-x-1.5"
          >
            <Scale className="w-3.5 h-3.5" />
            <span>{isStaging ? "Staging..." : "Stage for Fleet Signature"}</span>
          </button>
        </div>
      </div>

      {notification && (
        <div className="p-3.5 rounded-xl bg-status-verified-bg border border-status-verified/30 text-xs text-status-verified flex items-center space-x-2">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          <span>{notification}</span>
        </div>
      )}

      {/* Staged Policy Signature Banner */}
      {selectedPolicy && selectedPolicy.status === "staged" && (
        <div className="p-6 rounded-2xl bg-status-warning-bg border border-status-warning/40 space-y-4">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div className="flex items-center space-x-3">
              <div className="w-12 h-12 rounded-xl bg-status-warning/20 text-status-warning flex items-center justify-center border border-status-warning/30 shrink-0">
                <Lock className="w-6 h-6" />
              </div>
              <div>
                <h3 className="text-base font-semibold text-text-primary flex items-center space-x-2">
                  <span>Policy Awaiting Ed25519 Signature</span>
                  <span className="font-mono text-xs text-status-warning px-2 py-0.5 rounded bg-status-warning/10 border border-status-warning/20">
                    STAGED
                  </span>
                </h3>
                <p className="text-xs text-text-secondary">
                  The Cloud API cannot push this policy to nodes until an authorized operator verifies the diff and signs it with their private key in Rivun Control.
                </p>
              </div>
            </div>

            <button
              onClick={handleSimulateLocalSign}
              disabled={isSigning}
              className="px-5 py-2.5 rounded-xl bg-status-warning text-black font-semibold text-xs hover:bg-status-warning/90 transition shadow-sm flex items-center space-x-2 shrink-0"
            >
              <Lock className="w-4 h-4" />
              <span>{isSigning ? "Signing locally..." : "Sign with Operator Key (Rivun Control)"}</span>
            </button>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex items-center space-x-2 border-b border-border-subtle pb-3">
        <button
          onClick={() => setActiveTab("visual")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "visual"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Visual Rule Builder
        </button>
        <button
          onClick={() => setActiveTab("diff")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition flex items-center space-x-1.5 ${
            activeTab === "diff"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          <Code className="w-3.5 h-3.5" />
          <span>Active vs Staged Diff</span>
        </button>
        <button
          onClick={() => setActiveTab("toml")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "toml"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          TOML Manifest
        </button>
      </div>

      {activeTab === "visual" && (
        <div className="space-y-6">
          {/* Policy Name & Default Decision */}
          <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle flex flex-col sm:flex-row sm:items-center justify-between gap-4">
            <div className="space-y-1">
              <label className="text-xs font-medium text-text-secondary">Policy Bundle Name</label>
              <input
                type="text"
                value={policyName}
                onChange={(e: any) => setPolicyName(e.target.value)}
                className="w-full sm:w-80 px-3 py-1.5 rounded-lg bg-bg-surface-raised border border-border-subtle text-xs text-text-primary font-mono focus:outline-none focus:border-accent-primary"
              />
            </div>

            <div className="flex items-center space-x-3 text-xs">
              <span className="text-text-secondary">Default Decision:</span>
              <span className="font-mono text-status-critical font-semibold px-2.5 py-1 rounded bg-status-critical-bg border border-status-critical/30">
                DENY (Zero-Trust Fail-Closed)
              </span>
            </div>
          </div>

          {/* Rules List */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-sm font-semibold text-text-primary uppercase tracking-wide font-mono">
                Policy Rules Evaluation Sequence
              </h3>
              <button
                onClick={handleAddRule}
                className="px-3 py-1.5 rounded-lg bg-bg-surface border border-border-subtle hover:border-accent-primary text-xs font-medium text-text-primary transition flex items-center space-x-1"
              >
                <Plus className="w-3.5 h-3.5 text-accent-primary" />
                <span>Add Condition Block</span>
              </button>
            </div>

            <div className="space-y-3">
              {rules.map((rule, idx) => (
                <div
                  key={rule.id}
                  className="p-4 rounded-xl bg-bg-surface border border-border-subtle hover:border-border-strong transition flex flex-col md:flex-row md:items-center justify-between gap-4"
                >
                  <div className="flex items-center space-x-4">
                    <span className="w-6 h-6 rounded-full bg-bg-surface-raised border border-border-subtle flex items-center justify-center font-mono text-[11px] text-text-secondary shrink-0">
                      {idx + 1}
                    </span>
                    <div className="space-y-1">
                      <div className="flex items-center space-x-2">
                        <span className="font-semibold text-text-primary text-xs">{rule.name}</span>
                        {rule.subject && (
                          <span className="font-mono text-[11px] px-2 py-0.5 rounded bg-bg-surface-raised text-accent-primary border border-border-subtle">
                            subject: {rule.subject}
                          </span>
                        )}
                        {rule.kind && (
                          <span className="font-mono text-[11px] px-2 py-0.5 rounded bg-bg-surface-raised text-text-secondary border border-border-subtle">
                            kind: {rule.kind}
                          </span>
                        )}
                      </div>
                      {rule.required_capability && (
                        <div className="text-[11px] text-text-secondary font-mono">
                          Requires Capability: <strong className="text-text-primary">{rule.required_capability}</strong>
                        </div>
                      )}
                    </div>
                  </div>

                  <div className="flex items-center space-x-3 shrink-0">
                    <span
                      className={`text-xs font-mono px-3 py-1 rounded-full font-semibold uppercase tracking-wider ${
                        rule.decision === "allow"
                          ? "bg-status-verified-bg text-status-verified border border-status-verified/20"
                          : rule.decision === "require_poa"
                          ? "bg-purple-500/10 text-purple-400 border border-purple-500/20"
                          : "bg-blue-500/10 text-blue-400 border border-blue-500/20"
                      }`}
                    >
                      {rule.decision.replace(/_/g, " ")}
                    </span>

                    <button
                      onClick={() => handleRemoveRule(rule.id)}
                      className="p-1.5 rounded-lg text-text-muted hover:text-status-critical hover:bg-bg-surface-raised transition"
                    >
                      <Trash2 className="w-4 h-4" />
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {activeTab === "diff" && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs text-text-secondary font-mono">
              <span>ACTIVE FLEET POLICY (v1)</span>
              <span className="text-status-verified font-semibold">Live on Nodes</span>
            </div>
            <pre className="p-4 rounded-xl bg-bg-surface border border-border-subtle text-[11px] font-mono text-text-secondary leading-relaxed overflow-x-auto h-96">
              {activePolicy?.body_toml || "No active policy"}
            </pre>
          </div>

          <div className="space-y-2">
            <div className="flex items-center justify-between text-xs text-text-secondary font-mono">
              <span>STAGED PROPOSAL (v2)</span>
              <span className="text-status-warning font-semibold">Awaiting Signature</span>
            </div>
            <pre className="p-4 rounded-xl bg-bg-surface-raised border border-status-warning/30 text-[11px] font-mono text-text-primary leading-relaxed overflow-x-auto h-96">
              {selectedPolicy?.body_toml || "No staged policy"}
            </pre>
          </div>
        </div>
      )}

      {activeTab === "toml" && (
        <div className="rounded-2xl bg-bg-surface border border-border-subtle p-6 space-y-3">
          <div className="text-xs font-semibold text-text-primary font-mono uppercase">
            Raw Deterministic TOML Manifest
          </div>
          <pre className="p-4 rounded-xl bg-bg-base border border-border-subtle text-xs font-mono text-accent-primary overflow-x-auto leading-relaxed">
            {selectedPolicy?.body_toml || "No policy selected"}
          </pre>
        </div>
      )}
    </div>
  );
}
