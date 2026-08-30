"use client";

import React, { useEffect, useState } from "react";
import {
  AlertTriangle,
  ShieldCheck,
  CheckCircle2,
  Download,
  Terminal,
  Activity,
  Layers,
} from "lucide-react";
import { api } from "../../lib/api";
import { IncidentRecord } from "../../lib/types";

export default function IncidentsPage() {
  const [incidents, setIncidents] = useState<IncidentRecord[]>([]);
  const [selectedIncident, setSelectedIncident] = useState<IncidentRecord | null>(null);

  useEffect(() => {
    async function load() {
      const data = await api.fetchIncidents();
      setIncidents(data);
      if (data.length > 0) setSelectedIncident(data[0]);
    }
    load();
  }, []);

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Incidents & Forensics</h1>
          <p className="text-sm text-text-secondary">
            Incident evidence snapshots with automated client-side secret scrubbing via <code className="font-mono text-accent-primary">SecretRedactor</code>.
          </p>
        </div>

        <div className="flex items-center space-x-2 text-xs font-mono text-status-verified bg-bg-surface px-3 py-1.5 rounded-lg border border-border-subtle">
          <ShieldCheck className="w-4 h-4" />
          <span>Zero Plaintext Secrets Ingested</span>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-8 items-start">
        {/* Incidents List (1 Col) */}
        <div className="space-y-3">
          <div className="text-xs font-semibold text-text-secondary uppercase tracking-wider font-mono px-1">
            Incident Timeline
          </div>

          <div className="space-y-3">
            {incidents.map((inc) => {
              const isSelected = selectedIncident?.id === inc.id;
              return (
                <div
                  key={inc.id}
                  onClick={() => setSelectedIncident(inc)}
                  className={`p-4 rounded-2xl border cursor-pointer transition space-y-2 ${
                    isSelected
                      ? "bg-bg-surface-raised border-accent-primary shadow-glow"
                      : "bg-bg-surface border-border-subtle hover:border-border-strong hover:bg-bg-surface-raised"
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <span
                      className={`text-xs font-mono px-2 py-0.5 rounded-full font-semibold uppercase ${
                        inc.severity === "critical"
                          ? "bg-status-critical-bg text-status-critical"
                          : "bg-status-warning-bg text-status-warning"
                      }`}
                    >
                      {inc.severity}
                    </span>
                    <span className="text-[10px] text-text-muted font-mono">
                      {new Date(inc.created_at).toLocaleTimeString()}
                    </span>
                  </div>

                  <div className="text-xs font-semibold text-text-primary">{inc.snapshot.reason}</div>

                  <div className="flex items-center justify-between text-[11px] text-text-secondary pt-2 border-t border-border-subtle">
                    <span>Node: <strong className="font-mono text-text-primary">{inc.node_label}</strong></span>
                    <span className="text-status-verified text-[10px]">Scrubbed</span>
                  </div>
                </div>
              );
            })}
          </div>
        </div>

        {/* Selected Incident Evidence Explorer (2 Cols) */}
        {selectedIncident && (
          <div className="lg:col-span-2 rounded-2xl bg-bg-surface border border-border-subtle p-6 space-y-6 shadow-card">
            <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 border-b border-border-subtle pb-4">
              <div>
                <div className="flex items-center space-x-2">
                  <span className="text-xs font-mono uppercase font-semibold text-status-warning px-2 py-0.5 rounded bg-status-warning-bg">
                    {selectedIncident.severity}
                  </span>
                  <h3 className="text-base font-semibold text-text-primary">{selectedIncident.snapshot.reason}</h3>
                </div>
                <div className="text-xs text-text-secondary font-mono mt-1">
                  Node: {selectedIncident.node_label} ({selectedIncident.node_id})
                </div>
              </div>

              <button className="px-3.5 py-1.5 rounded-lg bg-bg-surface-raised border border-border-subtle hover:border-border-strong text-xs font-semibold text-text-primary transition flex items-center space-x-1.5 shrink-0">
                <Download className="w-3.5 h-3.5 text-accent-primary" />
                <span>Export Redacted Tarball</span>
              </button>
            </div>

            {/* Redacted Snapshot Details */}
            <div className="space-y-3">
              <div className="text-xs font-semibold text-text-primary uppercase tracking-wide font-mono flex items-center space-x-2">
                <Layers className="w-4 h-4 text-accent-primary" />
                <span>Evidence Snapshot Payload</span>
              </div>
              <pre className="p-4 rounded-xl bg-bg-base border border-border-subtle text-xs font-mono text-text-primary overflow-x-auto leading-relaxed">
                {JSON.stringify(selectedIncident.snapshot, null, 2)}
              </pre>
            </div>

            {/* Redaction Guarantee Notice */}
            <div className="p-4 rounded-xl bg-bg-surface-raised border border-border-subtle text-xs text-text-secondary leading-relaxed space-y-1">
              <div className="font-semibold text-text-primary flex items-center space-x-2">
                <CheckCircle2 className="w-4 h-4 text-status-verified" />
                <span>Zero Private Key Leak Invariant Enforced</span>
              </div>
              <p>
                The edge node's <code className="font-mono text-accent-primary">SecretRedactor</code> stripped all private keys, passwords, bearer tokens, and confidential payloads before generating this evidence bundle.
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
