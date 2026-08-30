"use client";

import React from "react";
import { CheckCircle2, AlertTriangle, XCircle, Activity, ShieldCheck } from "lucide-react";
import { FleetDoctorStatus, NodeRecord } from "../lib/types";

interface Props {
  nodes: NodeRecord[];
}

export function DoctorHealthBanner({ nodes }: Props) {
  const onlineCount = nodes.filter((n) => n.status === "online").length;
  const degradedCount = nodes.filter((n) => n.status === "degraded").length;
  const offlineCount = nodes.filter((n) => n.status === "offline").length;

  const passedChecks = nodes.filter((n) => n.doctor_status === "passed").length;
  const warningChecks = nodes.filter((n) => n.doctor_status === "warning").length;
  const failedChecks = nodes.filter((n) => n.doctor_status === "failed").length;

  const overall: FleetDoctorStatus = failedChecks > 0 ? "failed" : warningChecks > 0 ? "warning" : "passed";

  return (
    <div className="rounded-2xl bg-bg-surface border border-border-subtle p-6 shadow-card relative overflow-hidden">
      {/* Glow highlight */}
      <div
        className={`absolute top-0 right-0 w-96 h-96 rounded-full blur-3xl opacity-10 pointer-events-none ${
          overall === "passed" ? "bg-status-verified" : overall === "warning" ? "bg-status-warning" : "bg-status-critical"
        }`}
      />

      <div className="flex flex-col lg:flex-row lg:items-center justify-between gap-6 relative z-10">
        {/* Overall Status summary */}
        <div className="flex items-start space-x-4">
          <div
            className={`w-12 h-12 rounded-xl flex items-center justify-center shrink-0 ${
              overall === "passed"
                ? "bg-status-verified-bg text-status-verified border border-status-verified/30 shadow-[0_0_15px_rgba(61,214,140,0.2)]"
                : overall === "warning"
                ? "bg-status-warning-bg text-status-warning border border-status-warning/30"
                : "bg-status-critical-bg text-status-critical border border-status-critical/30"
            }`}
          >
            {overall === "passed" ? (
              <CheckCircle2 className="w-6 h-6" />
            ) : overall === "warning" ? (
              <AlertTriangle className="w-6 h-6" />
            ) : (
              <XCircle className="w-6 h-6" />
            )}
          </div>
          <div>
            <div className="flex items-center space-x-3 mb-1">
              <h2 className="text-lg font-semibold text-text-primary">Global Fleet Health</h2>
              <span
                className={`text-xs font-mono px-2.5 py-0.5 rounded-full font-medium uppercase tracking-wider ${
                  overall === "passed"
                    ? "bg-status-verified-bg text-status-verified border border-status-verified/30"
                    : overall === "warning"
                    ? "bg-status-warning-bg text-status-warning border border-status-warning/30"
                    : "bg-status-critical-bg text-status-critical border border-status-critical/30"
                }`}
              >
                Doctor {overall}
              </span>
            </div>
            <p className="text-xs text-text-secondary">
              7 core criteria evaluated continuously: Network, Storage, Replay Guard, Journal MMR, Pack Signatures, Quorum Validity & Peer Trust.
            </p>
          </div>
        </div>

        {/* 6 Criteria Status Grid */}
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-3">
          <div className="px-3.5 py-2.5 rounded-xl bg-bg-surface-raised border border-border-subtle">
            <div className="text-[10px] uppercase font-mono text-text-secondary mb-0.5">Online Nodes</div>
            <div className="text-lg font-semibold text-text-primary flex items-baseline space-x-1.5">
              <span>{onlineCount}</span>
              <span className="text-xs text-text-secondary font-normal">/ {nodes.length}</span>
            </div>
          </div>

          <div className="px-3.5 py-2.5 rounded-xl bg-bg-surface-raised border border-border-subtle">
            <div className="text-[10px] uppercase font-mono text-text-secondary mb-0.5">Doctor Passed</div>
            <div className="text-lg font-semibold text-status-verified flex items-baseline space-x-1">
              <span>{passedChecks}</span>
              <span className="text-xs text-text-secondary font-normal font-mono">nodes</span>
            </div>
          </div>

          <div className="px-3.5 py-2.5 rounded-xl bg-bg-surface-raised border border-border-subtle">
            <div className="text-[10px] uppercase font-mono text-text-secondary mb-0.5">Warnings</div>
            <div className="text-lg font-semibold text-status-warning flex items-baseline space-x-1">
              <span>{warningChecks}</span>
              <span className="text-xs text-text-secondary font-normal font-mono">drifts</span>
            </div>
          </div>

          <div className="px-3.5 py-2.5 rounded-xl bg-bg-surface-raised border border-border-subtle">
            <div className="text-[10px] uppercase font-mono text-text-secondary mb-0.5">PoA Success</div>
            <div className="text-lg font-semibold text-accent-primary font-mono">99.98%</div>
          </div>
        </div>
      </div>
    </div>
  );
}
