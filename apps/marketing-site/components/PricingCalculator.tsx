"use client";

import React, { useState } from "react";
import {
  Check,
  Zap,
  Shield,
  HelpCircle,
  ArrowRight,
  Calculator,
  Sliders,
  DollarSign,
  TrendingUp,
  Sparkles,
} from "lucide-react";
import { PricingTier } from "../lib/types";

const PRICING_TIERS: PricingTier[] = [
  {
    id: "community",
    name: "Community",
    priceMonthly: 0,
    priceAnnual: 0,
    description: "Open source, zero-cost foundation for developers and local edge automation.",
    features: [
      "All 26 workspace Rust crates",
      "Unlimited local edge nodes",
      "Official SDKs (Rust, TS, Python, Go)",
      "7 Official Domain Packs",
      "Local CLI & Fleet Doctor tools",
      "Community GitHub / Discord support",
    ],
    ctaLabel: "Get Started Free",
  },
  {
    id: "pro",
    name: "Pro Team",
    badge: "MOST POPULAR",
    popular: true,
    priceMonthly: 49,
    priceAnnual: 39,
    description: "Multi-node fleet visibility and managed receipts indexing for growing teams.",
    features: [
      "Up to 25 connected edge nodes",
      "1,000,000 verified receipts / mo",
      "Rivun Cloud SaaS control plane",
      "30-day Merkle receipts retention",
      "Operator workstation key pairing",
      "Slack / Email priority support",
    ],
    ctaLabel: "Start 14-Day Free Trial",
  },
  {
    id: "enterprise",
    name: "Enterprise",
    priceMonthly: 499,
    priceAnnual: 399,
    description: "Deterministic BFT consensus, SOC2 compliance, and dedicated SLA guarantees.",
    features: [
      "Up to 250 connected edge nodes",
      "50,000,000 verified receipts / mo",
      "Multi-region BFT consensus quorums",
      "Custom domain pack authoring",
      "SAML / SSO & RBAC team roles",
      "24/7 dedicated support & <0.8ms SLA",
    ],
    ctaLabel: "Upgrade to Enterprise",
  },
  {
    id: "sovereign",
    name: "Sovereign Cloud",
    badge: "CUSTOM DEPLOYMENT",
    priceMonthly: 0,
    priceAnnual: 0,
    description: "Fully air-gapped, on-premise multi-tenant control plane for high-security environments.",
    features: [
      "Unlimited nodes & receipts volume",
      "Self-hosted Kubernetes control plane",
      "Hardware Security Module (HSM) keys",
      "Custom cryptographic signature curves",
      "Dedicated solutions architect",
      "Custom SLA guarantees & audit reviews",
    ],
    ctaLabel: "Contact Sovereign Team",
  },
];

export function PricingCalculator() {
  const [annualBilling, setAnnualBilling] = useState(true);
  const [nodeCount, setNodeCount] = useState(50);
  const [receiptsMillions, setReceiptsMillions] = useState(10); // in millions

  // ROI Calculator Math:
  // Legacy JSON-RPC message size: ~2.4 KB
  // Rivun Binary Wire size: ~140 Bytes (94% reduction)
  // Compute & Broker bandwidth savings
  const legacyBandwidthGb = (receiptsMillions * 1_000_000 * 2400) / (1024 * 1024 * 1024);
  const rivunBandwidthGb = (receiptsMillions * 1_000_000 * 140) / (1024 * 1024 * 1024);
  const bandwidthSavedGb = Math.max(0, legacyBandwidthGb - rivunBandwidthGb);

  const legacyBrokerCost = Math.round(receiptsMillions * 35 + nodeCount * 4);
  const rivunCost = Math.round(receiptsMillions * 6 + nodeCount * 1.5);
  const monthlySavings = Math.max(120, legacyBrokerCost - rivunCost);
  const annualSavings = monthlySavings * 12;

  return (
    <section id="pricing" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <DollarSign className="w-3.5 h-3.5" />
            <span>TRANSPARENT PRICING & ROI</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Predictable Scaling for Every Autonomous Fleet
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Start free with open source tools, or scale effortlessly with Rivun Cloud managed receipts.
            Switch to annual billing to save 20%.
          </p>

          {/* Billing Interval Toggle */}
          <div className="pt-4 flex items-center justify-center gap-3">
            <span className={`text-xs font-medium ${!annualBilling ? "text-white" : "text-[#9AA1AE]"}`}>
              Monthly Billing
            </span>
            <button
              onClick={() => setAnnualBilling(!annualBilling)}
              className="w-12 h-6 rounded-full bg-[#181B22] border border-[#22262F] p-1 flex items-center transition-colors"
            >
              <div
                className={`w-4 h-4 rounded-full bg-[#5B8CFF] transition-transform ${
                  annualBilling ? "translate-x-6" : "translate-x-0"
                }`}
              />
            </button>
            <span className={`text-xs font-medium flex items-center gap-1.5 ${annualBilling ? "text-white" : "text-[#9AA1AE]"}`}>
              <span>Annual Billing</span>
              <span className="px-2 py-0.5 rounded-full text-[10px] font-bold bg-[#3DD68C]/15 text-[#3DD68C] border border-[#3DD68C]/30">
                SAVE 20%
              </span>
            </span>
          </div>
        </div>

        {/* 4 Pricing Cards */}
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6 mb-16">
          {PRICING_TIERS.map((tier) => {
            const price = annualBilling ? tier.priceAnnual : tier.priceMonthly;
            return (
              <div
                key={tier.id}
                className={`p-6 rounded-2xl bg-[#111318] border flex flex-col justify-between transition-all duration-300 relative ${
                  tier.popular
                    ? "border-[#5B8CFF] shadow-glow scale-[1.02] bg-[#14171F]"
                    : "border-[#22262F] hover:border-[#3A4150]"
                }`}
              >
                {tier.badge && (
                  <div className="absolute -top-3 right-4 px-2.5 py-0.5 rounded-full bg-[#5B8CFF] text-[10px] font-mono font-bold text-white shadow-sm">
                    {tier.badge}
                  </div>
                )}

                <div>
                  <h3 className="text-lg font-bold text-white mb-1">{tier.name}</h3>
                  <p className="text-xs text-[#9AA1AE] min-h-[32px] mb-4">{tier.description}</p>

                  <div className="flex items-baseline gap-1 mb-6">
                    {tier.id === "sovereign" ? (
                      <span className="text-2xl font-bold text-white">Custom</span>
                    ) : (
                      <>
                        <span className="text-3xl font-extrabold text-white">${price}</span>
                        <span className="text-xs text-[#9AA1AE]">/ month</span>
                      </>
                    )}
                  </div>

                  <div className="space-y-2.5 pt-4 border-t border-[#22262F] mb-6">
                    {tier.features.map((feat, fIdx) => (
                      <div key={fIdx} className="flex items-start gap-2 text-xs text-[#9AA1AE]">
                        <Check className="w-4 h-4 text-[#3DD68C] shrink-0 mt-0.5" />
                        <span>{feat}</span>
                      </div>
                    ))}
                  </div>
                </div>

                <a
                  href={tier.id === "community" ? "/sandbox" : "#"}
                  className={`w-full py-2.5 rounded-xl text-xs font-semibold text-center transition-all flex items-center justify-center gap-1.5 ${
                    tier.popular
                      ? "bg-[#5B8CFF] hover:bg-[#4378F0] text-white shadow-glow"
                      : "bg-[#181B22] hover:bg-[#22262F] text-white border border-[#22262F]"
                  }`}
                >
                  <span>{tier.ctaLabel}</span>
                  <ArrowRight className="w-3.5 h-3.5" />
                </a>
              </div>
            );
          })}
        </div>

        {/* Interactive ROI & Infrastructure Cost Calculator */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 lg:p-10 shadow-2xl">
          <div className="flex items-center gap-3 pb-6 border-b border-[#22262F] mb-8">
            <div className="p-2.5 rounded-xl bg-[#3DD68C]/10 text-[#3DD68C] border border-[#3DD68C]/20">
              <TrendingUp className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-lg font-bold text-white">
                Interactive ROI & Bandwidth Savings Calculator
              </h3>
              <p className="text-xs text-[#9AA1AE]">
                Calculate your direct cloud compute and bandwidth savings vs legacy JSON-RPC and message brokers
              </p>
            </div>
          </div>

          <div className="grid grid-cols-1 lg:grid-cols-12 gap-8 items-center">
            {/* Sliders (7 cols) */}
            <div className="lg:col-span-7 space-y-6">
              {/* Slider 1: Nodes */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs font-semibold text-white">
                    Active Edge & Agent Nodes
                  </label>
                  <span className="text-xs font-mono font-bold text-[#5B8CFF] bg-[#181B22] px-2.5 py-1 rounded-lg border border-[#22262F]">
                    {nodeCount} Nodes
                  </span>
                </div>
                <input
                  type="range"
                  min={5}
                  max={500}
                  step={5}
                  value={nodeCount}
                  onChange={(e) => setNodeCount(Number(e.target.value))}
                  className="w-full accent-[#5B8CFF] cursor-pointer"
                />
                <div className="flex justify-between text-[10px] text-[#6B7280] font-mono mt-1">
                  <span>5 nodes</span>
                  <span>250 nodes</span>
                  <span>500+ nodes</span>
                </div>
              </div>

              {/* Slider 2: Receipts */}
              <div>
                <div className="flex items-center justify-between mb-2">
                  <label className="text-xs font-semibold text-white">
                    Monthly Action Receipts Volume
                  </label>
                  <span className="text-xs font-mono font-bold text-[#3DD68C] bg-[#181B22] px-2.5 py-1 rounded-lg border border-[#22262F]">
                    {receiptsMillions}M Receipts / mo
                  </span>
                </div>
                <input
                  type="range"
                  min={1}
                  max={100}
                  step={1}
                  value={receiptsMillions}
                  onChange={(e) => setReceiptsMillions(Number(e.target.value))}
                  className="w-full accent-[#3DD68C] cursor-pointer"
                />
                <div className="flex justify-between text-[10px] text-[#6B7280] font-mono mt-1">
                  <span>1M / mo</span>
                  <span>50M / mo</span>
                  <span>100M+ / mo</span>
                </div>
              </div>

              <div className="grid grid-cols-2 gap-3 pt-2 text-xs font-mono">
                <div className="p-3 rounded-lg bg-[#14171F] border border-[#22262F]">
                  <span className="text-[#6B7280] block text-[10px] uppercase">Bandwidth Saved</span>
                  <span className="text-white font-bold">{bandwidthSavedGb.toFixed(1)} GB / mo</span>
                </div>
                <div className="p-3 rounded-lg bg-[#14171F] border border-[#22262F]">
                  <span className="text-[#6B7280] block text-[10px] uppercase">Payload Compression</span>
                  <span className="text-[#3DD68C] font-bold">94.2% Smaller</span>
                </div>
              </div>
            </div>

            {/* Savings Callout Box (5 cols) */}
            <div className="lg:col-span-5 p-6 rounded-xl bg-[#0A0B0D] border border-[#3DD68C]/30 shadow-glow-emerald flex flex-col justify-between text-center space-y-4">
              <div>
                <span className="text-[10px] font-mono font-bold text-[#3DD68C] uppercase tracking-wider block mb-1">
                  ESTIMATED NET SAVINGS
                </span>
                <div className="text-4xl font-extrabold text-white">
                  ${annualSavings.toLocaleString()}
                </div>
                <span className="text-xs text-[#9AA1AE]">saved per year on infrastructure & logs</span>
              </div>

              <div className="p-3 bg-[#111318] rounded-lg border border-[#22262F] text-[11px] text-[#9AA1AE] text-left space-y-1">
                <div className="flex justify-between">
                  <span>Legacy Cloud Broker:</span>
                  <span className="text-rose-400 font-mono">${(legacyBrokerCost * 12).toLocaleString()}/yr</span>
                </div>
                <div className="flex justify-between">
                  <span>Rivun Binary Fabric:</span>
                  <span className="text-[#3DD68C] font-mono">${(rivunCost * 12).toLocaleString()}/yr</span>
                </div>
              </div>

              <a
                href="#innovations"
                className="w-full py-2.5 text-xs font-semibold text-black bg-[#3DD68C] hover:bg-[#34BE7B] rounded-lg transition-all flex items-center justify-center gap-1.5"
              >
                <span>Deploy Cost-Efficient Swarm</span>
                <ArrowRight className="w-3.5 h-3.5 text-black" />
              </a>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
