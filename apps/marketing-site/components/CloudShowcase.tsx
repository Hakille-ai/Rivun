"use client";

import React, { useState } from "react";
import {
  Cloud,
  Laptop,
  HardDrive,
  ShieldCheck,
  ArrowRight,
  Lock,
  Key,
  FileCheck,
  CheckCircle2,
  AlertCircle,
  Play,
  RotateCcw,
  Sparkles,
} from "lucide-react";

export function CloudShowcase() {
  const [currentStep, setCurrentStep] = useState(1);
  const [isSimulating, setIsSimulating] = useState(false);

  const steps = [
    {
      step: 1,
      title: "Visual Policy Drafting",
      actor: "Rivun Cloud SaaS",
      badgeColor: "text-[#5B8CFF] bg-[#5B8CFF]/15 border-[#5B8CFF]/30",
      description:
        "Engineers draft or update security policies in the web UI. Policies define fail-closed capability gates, rate limits, and consensus rules.",
      actionText: "Draft Rule: [[rules]] subject = 'repo.patch' decision = 'allow'",
      statusLabel: "Status: DRAFT (Pending Review)",
    },
    {
      step: 2,
      title: "Policy Staged in Cloud DB",
      actor: "Multi-Tenant Cloud API",
      badgeColor: "text-amber-400 bg-amber-500/15 border-amber-500/30",
      description:
        "The policy bundle is saved to the cloud database with status 'staged'. The cloud CANNOT activate or deploy the rule because it lacks the signing key.",
      actionText: "Bundle Hash: blake3:9a4b2c... | State: STAGED_INACTIVE",
      statusLabel: "Status: STAGED (Unsigned)",
    },
    {
      step: 3,
      title: "Local Offline Signing in Workstation",
      actor: "Operator Workstation (rivun-control)",
      badgeColor: "text-purple-400 bg-purple-500/15 border-purple-500/30",
      description:
        "The security operator opens the local desktop app (rivun-control), pulls the staged diff, reviews the AST, and signs the bundle with their private Ed25519 key stored in ~/.rivun/operator_keys/.",
      actionText: "Signed with Key ID: key_sec_ops_01 | Signature: 64B Ed25519",
      statusLabel: "Status: CRYPTOGRAPHICALLY SIGNED",
    },
    {
      step: 4,
      title: "Atomic Edge Fleet Deployment",
      actor: "Edge Fleet & Daemon Bridge",
      badgeColor: "text-[#3DD68C] bg-[#3DD68C]/15 border-[#3DD68C]/30",
      description:
        "The edge daemon bridge pulls the signed bundle, verifies the Ed25519 signature against its local trusted operator whitelist, and atomically swaps the policy on disk.",
      actionText: "Signature Verified: PASS | Atomic Swap: tempfile::persist()",
      statusLabel: "Status: LIVE IN PRODUCTION",
    },
  ];

  const handleNextStep = () => {
    if (currentStep < 4) {
      setCurrentStep((s) => s + 1);
    } else {
      setCurrentStep(1);
    }
  };

  const handleRunSimulation = () => {
    setIsSimulating(true);
    setCurrentStep(1);
    let step = 1;
    const interval = setInterval(() => {
      step++;
      if (step <= 4) {
        setCurrentStep(step);
      } else {
        clearInterval(interval);
        setIsSimulating(false);
      }
    }, 1200);
  };

  return (
    <section id="cloud" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <Cloud className="w-3.5 h-3.5" />
            <span>ZERO-TRUST CLOUD ARCHITECTURE</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Rivun Cloud SaaS & Sovereign Operator Workstation
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Never trust a cloud control plane with production keys. Rivun enforces complete isolation:
            the cloud manages telemetry and drafts, while the operator workstation holds the exclusive signing authority.
          </p>
        </div>

        {/* 3-Tier Architecture Diagram */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-12">
          {/* Box 1: SaaS Control Plane */}
          <div className="p-6 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-[#5B8CFF]/40 transition-all">
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2.5 rounded-xl bg-[#5B8CFF]/10 text-[#5B8CFF] border border-[#5B8CFF]/20">
                <Cloud className="w-5 h-5" />
              </div>
              <div>
                <h4 className="text-base font-bold text-white">Rivun Cloud SaaS</h4>
                <p className="text-xs text-[#9AA1AE]">Axum 0.8 REST & SSE</p>
              </div>
            </div>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#5B8CFF]" />
                <span>Multi-tenant telemetry & receipt indexing</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#5B8CFF]" />
                <span>Visual policy drafting & team collaboration</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#5B8CFF]" />
                <span>7-Point Fleet Doctor cluster health</span>
              </li>
            </ul>
          </div>

          {/* Box 2: Operator Workstation (Center with Key Vault) */}
          <div className="p-6 rounded-2xl bg-[#111318] border border-[#A855F7]/40 shadow-glow-purple relative group">
            <div className="absolute -top-3 right-4 px-2.5 py-0.5 rounded-full bg-[#A855F7]/20 border border-[#A855F7]/40 text-[10px] font-mono font-bold text-[#A855F7]">
              SOVEREIGN KEY VAULT
            </div>
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2.5 rounded-xl bg-purple-500/10 text-purple-400 border border-purple-500/20">
                <Laptop className="w-5 h-5" />
              </div>
              <div>
                <h4 className="text-base font-bold text-white">rivun-control Workstation</h4>
                <p className="text-xs text-[#9AA1AE]">Tauri Local Desktop App</p>
              </div>
            </div>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li className="flex items-center gap-2">
                <Key className="w-3.5 h-3.5 text-purple-400" />
                <span className="text-white font-medium">Local Ed25519 keys (~/.rivun/keys/)</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-purple-400" />
                <span>Air-gapped offline bundle signing</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-purple-400" />
                <span>Deterministic AST diff verification</span>
              </li>
            </ul>
          </div>

          {/* Box 3: Edge Fleets & Bridge */}
          <div className="p-6 rounded-2xl bg-[#111318] border border-[#22262F] relative group hover:border-[#3DD68C]/40 transition-all">
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2.5 rounded-xl bg-[#3DD68C]/10 text-[#3DD68C] border border-[#3DD68C]/20">
                <HardDrive className="w-5 h-5" />
              </div>
              <div>
                <h4 className="text-base font-bold text-white">Edge Fleet & Daemons</h4>
                <p className="text-xs text-[#9AA1AE]">rivun-cloud-bridge</p>
              </div>
            </div>
            <ul className="space-y-2 text-xs text-[#9AA1AE]">
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#3DD68C]" />
                <span>Autonomous edge action execution</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#3DD68C]" />
                <span>Signature check against local whitelist</span>
              </li>
              <li className="flex items-center gap-2">
                <CheckCircle2 className="w-3.5 h-3.5 text-[#3DD68C]" />
                <span>Fail-closed atomic filesystem swap</span>
              </li>
            </ul>
          </div>
        </div>

        {/* Interactive 4-Step Staging Simulator */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 lg:p-8 shadow-2xl">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-6 border-b border-[#22262F]">
            <div>
              <h3 className="text-lg font-bold text-white">
                Interactive 4-Step Staging & Offline Signing Workflow
              </h3>
              <p className="text-xs text-[#9AA1AE]">
                Simulate how enterprise security policies are drafted in the cloud and safely signed offline
              </p>
            </div>

            <div className="flex items-center gap-2">
              <button
                disabled={isSimulating}
                onClick={handleRunSimulation}
                className="px-4 py-2 text-xs font-semibold text-white bg-[#5B8CFF] hover:bg-[#4378F0] disabled:opacity-50 rounded-xl shadow-glow transition-all flex items-center gap-2"
              >
                <Play className="w-3.5 h-3.5" />
                <span>Auto-Run Simulation</span>
              </button>
              <button
                onClick={() => setCurrentStep(1)}
                className="p-2 rounded-xl bg-[#181B22] border border-[#22262F] text-[#9AA1AE] hover:text-white transition-all text-xs"
                title="Reset Simulation"
              >
                <RotateCcw className="w-4 h-4" />
              </button>
            </div>
          </div>

          {/* Stepper Navigation */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3 my-6">
            {steps.map((s) => (
              <button
                key={s.step}
                onClick={() => setCurrentStep(s.step)}
                className={`p-3.5 rounded-xl border text-left transition-all ${
                  currentStep === s.step
                    ? "bg-[#181B22] border-[#5B8CFF] shadow-glow"
                    : "bg-[#14171F] border-[#22262F] hover:border-[#3A4150]"
                }`}
              >
                <div className="flex items-center justify-between mb-1">
                  <span className="text-[10px] font-mono text-[#6B7280]">STEP 0{s.step}</span>
                  {currentStep > s.step && <CheckCircle2 className="w-3.5 h-3.5 text-[#3DD68C]" />}
                </div>
                <span className="text-xs font-bold text-white block">{s.title}</span>
                <span className="text-[10px] text-[#9AA1AE] truncate block mt-0.5">{s.actor}</span>
              </button>
            ))}
          </div>

          {/* Active Step Showcase Card */}
          <div className="p-6 rounded-xl bg-[#0A0B0D] border border-[#22262F] grid grid-cols-1 lg:grid-cols-12 gap-6 items-center">
            <div className="lg:col-span-8 space-y-3">
              <div className="flex items-center gap-2">
                <span className={`px-2.5 py-0.5 rounded text-[10px] font-mono font-bold border ${steps[currentStep - 1].badgeColor}`}>
                  {steps[currentStep - 1].actor}
                </span>
                <span className="text-xs font-mono text-[#3DD68C]">
                  {steps[currentStep - 1].statusLabel}
                </span>
              </div>

              <h4 className="text-base sm:text-lg font-bold text-white">
                {steps[currentStep - 1].title}
              </h4>

              <p className="text-xs sm:text-sm text-[#9AA1AE] leading-relaxed">
                {steps[currentStep - 1].description}
              </p>

              <div className="p-3 bg-[#111318] rounded-lg border border-[#22262F] font-mono text-xs text-[#5B8CFF] truncate">
                {steps[currentStep - 1].actionText}
              </div>
            </div>

            <div className="lg:col-span-4 flex flex-col items-center justify-center p-6 bg-[#14171F] rounded-xl border border-[#22262F] text-center space-y-3">
              <div className="w-12 h-12 rounded-full bg-[#5B8CFF]/15 border border-[#5B8CFF]/30 flex items-center justify-center text-[#5B8CFF]">
                {currentStep === 1 && <Cloud className="w-6 h-6" />}
                {currentStep === 2 && <Lock className="w-6 h-6 text-amber-400" />}
                {currentStep === 3 && <Key className="w-6 h-6 text-purple-400" />}
                {currentStep === 4 && <ShieldCheck className="w-6 h-6 text-[#3DD68C]" />}
              </div>

              <div>
                <span className="text-xs font-bold text-white block">
                  {currentStep === 4 ? "Deployment Complete" : "Advance Pipeline"}
                </span>
                <span className="text-[11px] text-[#6B7280]">
                  {currentStep === 4 ? "All nodes running verified bundle" : `Next: Step 0${(currentStep % 4) + 1}`}
                </span>
              </div>

              <button
                onClick={handleNextStep}
                className="w-full py-2 text-xs font-semibold text-white bg-[#181B22] hover:bg-[#22262F] border border-[#22262F] rounded-lg transition-all flex items-center justify-center gap-1.5"
              >
                <span>{currentStep === 4 ? "Restart Workflow" : "Proceed to Next Step"}</span>
                <ArrowRight className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
