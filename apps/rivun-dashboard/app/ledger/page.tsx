"use client";

import React, { useEffect, useState } from "react";
import {
  FileCheck,
  Search,
  Filter,
  ShieldCheck,
  CheckCircle2,
  Key,
  Layers,
  ArrowRight,
  ExternalLink,
  ChevronRight,
  X,
} from "lucide-react";
import { api } from "../../lib/api";
import { ProvenanceGraph } from "../../components/ProvenanceGraph";
import { OfflineVerifierModal } from "../../components/OfflineVerifierModal";
import { ReceiptRecord } from "../../lib/types";

export default function LedgerPage() {
  const [receipts, setReceipts] = useState<ReceiptRecord[]>([]);
  const [selectedReceipt, setSelectedReceipt] = useState<ReceiptRecord | null>(null);
  const [search, setSearch] = useState("");
  const [filterKind, setFilterKind] = useState<string>("all");
  const [filterPoa, setFilterPoa] = useState<string>("all");
  const [isVerifierOpen, setIsVerifierOpen] = useState(false);

  useEffect(() => {
    async function load() {
      const data = await api.fetchReceipts();
      setReceipts(data);
      if (data.length > 0 && !selectedReceipt) {
        setSelectedReceipt(data[0]);
      }
    }
    load();
  }, []);

  const filteredReceipts = receipts.filter((r) => {
    const matchesSearch =
      r.receipt_hash.toLowerCase().includes(search.toLowerCase()) ||
      r.action_kind.toLowerCase().includes(search.toLowerCase()) ||
      r.node_label.toLowerCase().includes(search.toLowerCase());
    const matchesKind = filterKind === "all" || r.action_kind.includes(filterKind);
    const matchesPoa = filterPoa === "all" || r.poa_status === filterPoa;
    return matchesSearch && matchesKind && matchesPoa;
  });

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Receipts & Provenance Ledger</h1>
          <p className="text-sm text-text-secondary">
            Immutable append-only cryptographic receipts with Merkle-Damgård chained provenance links.
          </p>
        </div>

        {selectedReceipt && (
          <button
            onClick={() => setIsVerifierOpen(true)}
            className="px-4 py-2 rounded-lg bg-bg-surface border border-border-subtle hover:border-accent-primary text-xs font-semibold text-text-primary transition flex items-center space-x-2 shadow-sm shrink-0"
          >
            <ShieldCheck className="w-4 h-4 text-status-verified" />
            <span>Verify Offline in CLI</span>
          </button>
        )}
      </div>

      {/* Selected Receipt Provenance Inspector */}
      {selectedReceipt && selectedReceipt.provenance_chain && (
        <div className="rounded-2xl bg-bg-surface border border-border-subtle p-6 space-y-4 shadow-card">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 border-b border-border-subtle pb-4">
            <div>
              <div className="flex items-center space-x-2">
                <span className="font-mono text-sm font-semibold text-text-primary">{selectedReceipt.action_kind}</span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded-full bg-status-verified-bg text-status-verified border border-status-verified/20">
                  PoA: {selectedReceipt.poa_status}
                </span>
              </div>
              <div className="text-xs text-text-secondary font-mono mt-0.5">
                Receipt Hash: {selectedReceipt.receipt_hash}
              </div>
            </div>

            <div className="text-xs text-text-secondary font-mono">
              Emitted by: <strong className="text-text-primary">{selectedReceipt.node_label}</strong> at{" "}
              {new Date(selectedReceipt.occurred_at).toLocaleTimeString()}
            </div>
          </div>

          <ProvenanceGraph
            provenance={selectedReceipt.provenance_chain}
            onVerifyOffline={() => setIsVerifierOpen(true)}
          />
        </div>
      )}

      {/* Search & Filters */}
      <div className="flex flex-col sm:flex-row gap-3">
        <div className="flex-1 relative">
          <Search className="w-4 h-4 text-text-muted absolute left-3.5 top-1/2 -translate-y-1/2" />
          <input
            type="text"
            placeholder="Search receipts by hash, action subject, or node..."
            value={search}
            onChange={(e: any) => setSearch(e.target.value)}
            className="w-full pl-10 pr-4 py-2 rounded-xl bg-bg-surface border border-border-subtle text-xs text-text-primary placeholder:text-text-muted focus:outline-none focus:border-accent-primary transition"
          />
        </div>

        <div className="flex items-center space-x-3">
          <select
            value={filterKind}
            onChange={(e: any) => setFilterKind(e.target.value)}
            className="px-3 py-2 rounded-xl bg-bg-surface border border-border-subtle text-xs text-text-primary focus:outline-none focus:border-accent-primary"
          >
            <option value="all">All Action Kinds</option>
            <option value="smart_building">Smart Building</option>
            <option value="safety">Safety Controls</option>
            <option value="settlement">Escrow Settlement</option>
            <option value="driver">Driver Execution</option>
          </select>

          <select
            value={filterPoa}
            onChange={(e: any) => setFilterPoa(e.target.value)}
            className="px-3 py-2 rounded-xl bg-bg-surface border border-border-subtle text-xs text-text-primary focus:outline-none focus:border-accent-primary"
          >
            <option value="all">All Consensus States</option>
            <option value="verified">PoA Verified</option>
            <option value="none">Single Node (None)</option>
          </select>
        </div>
      </div>

      {/* High-density Receipts Ledger Table */}
      <div className="rounded-2xl bg-bg-surface border border-border-subtle overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-border-subtle bg-bg-surface-raised text-text-secondary font-mono uppercase tracking-wider text-[10px]">
                <th className="py-3 px-4">Receipt Hash</th>
                <th className="py-3 px-4">Action Kind / Subject</th>
                <th className="py-3 px-4">PoA Status</th>
                <th className="py-3 px-4">Origin Node</th>
                <th className="py-3 px-4">Timestamp</th>
                <th className="py-3 px-4 text-right">Integrity</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle">
              {filteredReceipts.map((r) => {
                const isSelected = selectedReceipt?.id === r.id;
                return (
                  <tr
                    key={r.id}
                    onClick={() => setSelectedReceipt(r)}
                    className={`cursor-pointer transition ${
                      isSelected
                        ? "bg-accent-glow border-l-2 border-accent-primary"
                        : "hover:bg-bg-surface-raised"
                    }`}
                  >
                    <td className="py-3.5 px-4 font-mono font-medium text-text-primary">
                      {r.receipt_hash.slice(0, 18)}...
                    </td>

                    <td className="py-3.5 px-4">
                      <span className="font-mono text-text-primary font-medium">{r.action_kind}</span>
                    </td>

                    <td className="py-3.5 px-4">
                      <span
                        className={`text-[10px] font-mono px-2 py-0.5 rounded-full ${
                          r.poa_status === "verified"
                            ? "bg-status-verified-bg text-status-verified border border-status-verified/20"
                            : "bg-bg-surface-raised text-text-secondary border border-border-subtle"
                        }`}
                      >
                        {r.poa_status}
                      </span>
                    </td>

                    <td className="py-3.5 px-4 font-mono text-text-secondary">{r.node_label}</td>

                    <td className="py-3.5 px-4 font-mono text-[11px] text-text-secondary">
                      {new Date(r.occurred_at).toLocaleTimeString()}
                    </td>

                    <td className="py-3.5 px-4 text-right">
                      <span className="text-[10px] font-mono text-status-verified font-medium flex items-center justify-end space-x-1">
                        <CheckCircle2 className="w-3.5 h-3.5" />
                        <span>MMR Peak Bag</span>
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>

      {/* Offline Verifier Modal */}
      {selectedReceipt && (
        <OfflineVerifierModal
          isOpen={isVerifierOpen}
          receiptHash={selectedReceipt.receipt_hash}
          onClose={() => setIsVerifierOpen(false)}
        />
      )}
    </div>
  );
}
