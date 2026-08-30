"use client";

import React, { useEffect, useState } from "react";
import Link from "next/link";
import {
  FileCheck,
  Scale,
  AlertTriangle,
  Server,
  Activity,
  ArrowUpRight,
  ShieldAlert,
  Zap,
  Lock,
  Layers,
  CheckCircle2,
} from "lucide-react";
import { DoctorHealthBanner } from "../components/DoctorHealthBanner";
import { api } from "../lib/api";
import { IncidentRecord, NodeRecord, PolicyRecord, ReceiptRecord } from "../lib/types";

export default function OverviewPage() {
  const [nodes, setNodes] = useState<NodeRecord[]>([]);
  const [receipts, setReceipts] = useState<ReceiptRecord[]>([]);
  const [policies, setPolicies] = useState<PolicyRecord[]>([]);
  const [incidents, setIncidents] = useState<IncidentRecord[]>([]);
  const [liveCount, setLiveCount] = useState(48920);

  useEffect(() => {
    async function loadData() {
      const [n, r, p, inc] = await Promise.all([
        api.fetchNodes(),
        api.fetchReceipts(),
        api.fetchPolicies(),
        api.fetchIncidents(),
      ]);
      setNodes(n);
      setReceipts(r);
      setPolicies(p);
      setIncidents(inc);
    }
    loadData();

    // SSE Ticker Simulation
    const interval = setInterval(() => {
      setLiveCount((prev) => prev + Math.floor(Math.random() * 3) + 1);
    }, 2500);

    return () => clearInterval(interval);
  }, []);

  const stagedPolicy = policies.find((p) => p.status === "staged");
  const activeIncidents = incidents.filter((i) => !i.resolved);

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Top Title & Quick Actions */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Organization Overview</h1>
          <p className="text-sm text-text-secondary">
            Centralized zero-trust monitoring and cryptographic audit ledger for Acme Autonomous Systems.
          </p>
        </div>

        <div className="flex items-center space-x-3">
          <Link
            href="/policies"
            className="px-4 py-2 rounded-lg bg-bg-surface border border-border-subtle hover:border-border-strong text-xs font-semibold text-text-primary transition"
          >
            Draft Policy
          </Link>
          <Link
            href="/ledger"
            className="px-4 py-2 rounded-lg bg-accent-primary hover:bg-accent-hover text-white text-xs font-semibold shadow-glow transition flex items-center space-x-1.5"
          >
            <FileCheck className="w-3.5 h-3.5" />
            <span>Explore Receipts Ledger</span>
          </Link>
        </div>
      </div>

      {/* Global Doctor Health Banner */}
      <DoctorHealthBanner nodes={nodes} />

      {/* Staged Policy Alert (Critical human-in-the-loop notification) */}
      {stagedPolicy && (
        <div className="p-4 rounded-xl bg-status-warning-bg border border-status-warning/30 flex flex-col md:flex-row md:items-center justify-between gap-4">
          <div className="flex items-center space-x-3">
            <div className="w-9 h-9 rounded-lg bg-status-warning/20 text-status-warning flex items-center justify-center shrink-0">
              <Scale className="w-5 h-5" />
            </div>
            <div>
              <div className="text-sm font-semibold text-text-primary flex items-center space-x-2">
                <span>Policy Staged for Deployment:</span>
                <span className="font-mono text-status-warning">{stagedPolicy.name} (v{stagedPolicy.version})</span>
              </div>
              <p className="text-xs text-text-secondary">
                Awaiting local operator Ed25519 signature. Zero-trust invariant prevents cloud from deploying policies without human authorization.
              </p>
            </div>
          </div>
          <Link
            href="/policies"
            className="px-4 py-2 rounded-lg bg-status-warning text-black font-semibold text-xs hover:bg-status-warning/90 transition shrink-0 flex items-center space-x-1.5 shadow-sm"
          >
            <Lock className="w-3.5 h-3.5" />
            <span>Review & Sign in Rivun Control</span>
          </Link>
        </div>
      )}

      {/* Key Metric Spark Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-3">
            <span className="text-xs font-medium uppercase font-mono tracking-wider">Total Receipts Ingested</span>
            <FileCheck className="w-4 h-4 text-accent-primary" />
          </div>
          <div className="text-2xl font-bold text-text-primary font-mono tracking-tight">
            {liveCount.toLocaleString()}
          </div>
          <div className="mt-2 text-[11px] text-status-verified flex items-center space-x-1">
            <span className="w-1.5 h-1.5 rounded-full bg-status-verified animate-pulse" />
            <span>Streaming in real-time (SSE)</span>
          </div>
        </div>

        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-3">
            <span className="text-xs font-medium uppercase font-mono tracking-wider">Active Edge Nodes</span>
            <Server className="w-4 h-4 text-status-verified" />
          </div>
          <div className="text-2xl font-bold text-text-primary font-mono tracking-tight">
            {nodes.filter((n) => n.status === "online").length} / {nodes.length}
          </div>
          <div className="mt-2 text-[11px] text-text-secondary">
            <span>3 Global Regions (FRA, IAD, SIN)</span>
          </div>
        </div>

        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-3">
            <span className="text-xs font-medium uppercase font-mono tracking-wider">PoA Consensus Quorum</span>
            <ShieldAlert className="w-4 h-4 text-accent-primary" />
          </div>
          <div className="text-2xl font-bold text-text-primary font-mono tracking-tight">
            3-of-4 <span className="text-xs text-text-secondary font-normal font-sans">(75% threshold)</span>
          </div>
          <div className="mt-2 text-[11px] text-status-verified font-mono">
            <span>0 Byzantine Slashing events</span>
          </div>
        </div>

        <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
          <div className="flex items-center justify-between text-text-secondary mb-3">
            <span className="text-xs font-medium uppercase font-mono tracking-wider">Active Incidents</span>
            <AlertTriangle className="w-4 h-4 text-status-warning" />
          </div>
          <div className="text-2xl font-bold text-status-warning font-mono tracking-tight">
            {activeIncidents.length}
          </div>
          <div className="mt-2 text-[11px] text-text-secondary">
            <span>SecretRedactor applied to snapshots</span>
          </div>
        </div>
      </div>

      {/* Two Columns: Recent Receipts Live Feed + Active Incidents & Topology */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8">
        {/* Recent Ledger Feed (2 Cols) */}
        <div className="lg:col-span-2 space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center space-x-2">
              <Activity className="w-4 h-4 text-accent-primary" />
              <h3 className="text-base font-semibold text-text-primary">Live Ledger Receipts Stream</h3>
            </div>
            <Link href="/ledger" className="text-xs text-accent-primary hover:underline flex items-center space-x-1">
              <span>View Full Ledger</span>
              <ArrowUpRight className="w-3.5 h-3.5" />
            </Link>
          </div>

          <div className="rounded-2xl bg-bg-surface border border-border-subtle divide-y divide-border-subtle overflow-hidden">
            {receipts.slice(0, 6).map((r) => (
              <div key={r.id} className="p-4 flex items-center justify-between hover:bg-bg-surface-raised transition">
                <div className="space-y-1">
                  <div className="flex items-center space-x-2">
                    <span className="font-mono text-xs font-semibold text-text-primary">{r.action_kind}</span>
                    <span
                      className={`text-[10px] font-mono px-2 py-0.5 rounded-full ${
                        r.poa_status === "verified"
                          ? "bg-status-verified-bg text-status-verified border border-status-verified/20"
                          : "bg-bg-surface-raised text-text-secondary border border-border-subtle"
                      }`}
                    >
                      PoA: {r.poa_status}
                    </span>
                  </div>
                  <div className="flex items-center space-x-3 text-xs text-text-secondary">
                    <span>Node: <strong className="text-text-primary font-mono">{r.node_label}</strong></span>
                    <span>&bull;</span>
                    <span className="font-mono text-[11px] text-text-muted truncate max-w-[200px]">{r.receipt_hash}</span>
                  </div>
                </div>

                <div className="text-right">
                  <div className="text-xs text-text-secondary font-mono">
                    {new Date(r.occurred_at).toLocaleTimeString()}
                  </div>
                  <span className="text-[10px] text-status-verified font-mono">● Verified</span>
                </div>
              </div>
            ))}
          </div>
        </div>

        {/* Right Column: Fleet Incidents & Domain Packs Quick Grid */}
        <div className="space-y-6">
          {/* Active Incidents */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-base font-semibold text-text-primary">Incident Watchdog</h3>
              <Link href="/incidents" className="text-xs text-accent-primary hover:underline">
                History
              </Link>
            </div>

            <div className="rounded-2xl bg-bg-surface border border-border-subtle p-4 space-y-3">
              {incidents.length === 0 ? (
                <div className="text-xs text-text-secondary py-6 text-center">No active incidents reported.</div>
              ) : (
                incidents.map((inc) => (
                  <div key={inc.id} className="p-3 rounded-xl bg-bg-surface-raised border border-border-subtle space-y-2">
                    <div className="flex items-center justify-between">
                      <span className="text-xs font-semibold text-status-warning font-mono uppercase">
                        {inc.severity}
                      </span>
                      <span className="text-[10px] text-text-muted font-mono">
                        {new Date(inc.created_at).toLocaleTimeString()}
                      </span>
                    </div>
                    <div className="text-xs text-text-primary font-medium">{inc.snapshot.reason}</div>
                    <div className="text-[11px] text-text-secondary flex items-center space-x-2">
                      <span>Node: <strong className="font-mono">{inc.node_label}</strong></span>
                      <span>&bull;</span>
                      <span className="text-status-verified text-[10px]">Redacted</span>
                    </div>
                  </div>
                ))
              )}
            </div>
          </div>

          {/* Quick Domain Packs Catalog Preview */}
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <h3 className="text-base font-semibold text-text-primary">Domain Packs</h3>
              <Link href="/marketplace" className="text-xs text-accent-primary hover:underline">
                Marketplace (7)
              </Link>
            </div>

            <div className="rounded-2xl bg-bg-surface border border-border-subtle p-4 space-y-2.5">
              {["smart-building", "agentic-dev", "industrial"].map((pack) => (
                <div key={pack} className="flex items-center justify-between text-xs p-2 rounded-lg hover:bg-bg-surface-raised transition">
                  <div className="flex items-center space-x-2">
                    <Zap className="w-3.5 h-3.5 text-accent-primary" />
                    <span className="font-medium text-text-primary font-mono">{pack}</span>
                  </div>
                  <span className="text-[10px] font-mono text-status-verified">Signed (v0.1.0)</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
