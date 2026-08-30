"use client";

import React, { useEffect, useState } from "react";
import {
  ShoppingBag,
  ShieldCheck,
  Download,
  Building,
  Bot,
  Cloud,
  Factory,
  HeartPulse,
  DollarSign,
  User,
  CheckCircle2,
  X,
} from "lucide-react";
import { api } from "../../lib/api";
import { PackRecord } from "../../lib/types";

export default function MarketplacePage() {
  const [packs, setPacks] = useState<PackRecord[]>([]);
  const [selectedPack, setSelectedPack] = useState<PackRecord | null>(null);
  const [isInstalling, setIsInstalling] = useState(false);
  const [installedSuccess, setInstalledSuccess] = useState(false);

  useEffect(() => {
    async function load() {
      const data = await api.fetchPacks();
      setPacks(data);
    }
    load();
  }, []);

  const getPackIcon = (name: string) => {
    switch (name) {
      case "smart-building":
        return Building;
      case "agentic-dev":
        return Bot;
      case "cloud-ops":
        return Cloud;
      case "industrial":
        return Factory;
      case "healthcare":
        return HeartPulse;
      case "finance":
        return DollarSign;
      default:
        return User;
    }
  };

  const handleInstall = () => {
    setIsInstalling(true);
    setTimeout(() => {
      setIsInstalling(false);
      setInstalledSuccess(true);
      setTimeout(() => {
        setInstalledSuccess(false);
        setSelectedPack(null);
      }, 2500);
    }, 1200);
  };

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Domain Pack Marketplace</h1>
          <p className="text-sm text-text-secondary">
            Verified ZAP domain packs with cryptographic manifests, WASM sandboxing, and fleet deployment.
          </p>
        </div>

        <div className="flex items-center space-x-2 text-xs font-mono text-text-secondary bg-bg-surface px-3 py-1.5 rounded-lg border border-border-subtle">
          <ShieldCheck className="w-4 h-4 text-status-verified" />
          <span>7 Verified Foundation Packs</span>
        </div>
      </div>

      {/* Packs Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
        {packs.map((pack) => {
          const Icon = getPackIcon(pack.name);
          return (
            <div
              key={pack.id}
              className="p-6 rounded-2xl bg-bg-surface border border-border-subtle hover:border-border-strong hover:bg-bg-surface-raised transition-all space-y-4 flex flex-col justify-between shadow-card relative group"
            >
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <div className="w-12 h-12 rounded-xl bg-bg-surface-raised border border-border-subtle flex items-center justify-center text-accent-primary group-hover:shadow-glow transition">
                    <Icon className="w-6 h-6" />
                  </div>
                  <span className="text-[10px] font-mono px-2.5 py-0.5 rounded-full bg-status-verified-bg text-status-verified border border-status-verified/20 font-medium">
                    Verified Manifest
                  </span>
                </div>

                <div>
                  <div className="flex items-center justify-between">
                    <h3 className="text-base font-semibold text-text-primary font-mono">{pack.name}</h3>
                    <span className="text-xs text-text-muted font-mono">v{pack.version}</span>
                  </div>
                  <div className="text-[11px] text-accent-primary font-mono mt-0.5">{pack.category}</div>
                </div>

                <p className="text-xs text-text-secondary leading-relaxed">{pack.description}</p>
              </div>

              <div className="pt-4 border-t border-border-subtle flex items-center justify-between">
                <div className="text-[11px] text-text-muted font-mono">
                  By {pack.author}
                </div>

                <button
                  onClick={() => setSelectedPack(pack)}
                  className="px-3.5 py-1.5 rounded-lg bg-bg-surface-raised hover:bg-accent-primary hover:text-white border border-border-subtle text-xs font-semibold text-text-primary transition flex items-center space-x-1.5"
                >
                  <Download className="w-3.5 h-3.5" />
                  <span>Deploy</span>
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {/* Deployment Modal */}
      {selectedPack && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm p-4 animate-in fade-in duration-200">
          <div className="w-full max-w-lg rounded-2xl bg-bg-surface-raised border border-border-strong p-6 shadow-modal space-y-6 relative">
            <button
              onClick={() => setSelectedPack(null)}
              className="absolute top-4 right-4 p-2 rounded-lg text-text-secondary hover:text-text-primary hover:bg-bg-surface transition"
            >
              <X className="w-4 h-4" />
            </button>

            <div className="flex items-center space-x-3">
              <div className="w-12 h-12 rounded-xl bg-accent-glow text-accent-primary flex items-center justify-center border border-accent-primary/20">
                <ShoppingBag className="w-6 h-6" />
              </div>
              <div>
                <h3 className="text-base font-semibold text-text-primary font-mono">{selectedPack.name}</h3>
                <div className="text-xs text-text-secondary">Deploy to Acme fleet nodes</div>
              </div>
            </div>

            <div className="space-y-3 bg-bg-base p-4 rounded-xl border border-border-subtle text-xs">
              <div className="flex justify-between">
                <span className="text-text-secondary">Manifest Hash:</span>
                <span className="font-mono text-text-primary">{selectedPack.manifest_hash}</span>
              </div>
              <div className="flex justify-between">
                <span className="text-text-secondary">Cryptographic Signer:</span>
                <span className="font-mono text-status-verified">Rivun Foundation Root (Ed25519)</span>
              </div>
              <div className="flex justify-between">
                <span className="text-text-secondary">Target Nodes:</span>
                <span className="font-semibold text-text-primary">All 5 Online Nodes (Auto-Sync)</span>
              </div>
            </div>

            {installedSuccess && (
              <div className="p-3.5 rounded-xl bg-status-verified-bg border border-status-verified/30 text-xs text-status-verified flex items-center space-x-2">
                <CheckCircle2 className="w-4 h-4 shrink-0" />
                <span>Pack installation dispatched! Edge bridges will verify signature and load WASM.</span>
              </div>
            )}

            <div className="flex justify-end space-x-3 pt-2">
              <button
                onClick={() => setSelectedPack(null)}
                className="px-4 py-2 rounded-lg bg-bg-surface border border-border-subtle text-xs text-text-secondary hover:text-text-primary transition"
              >
                Cancel
              </button>
              <button
                onClick={handleInstall}
                disabled={isInstalling || installedSuccess}
                className="px-5 py-2 rounded-lg bg-accent-primary text-white text-xs font-semibold hover:bg-accent-hover transition shadow-glow flex items-center space-x-2"
              >
                {isInstalling ? (
                  <span>Dispatching to fleet...</span>
                ) : installedSuccess ? (
                  <span>Dispatched!</span>
                ) : (
                  <span>Confirm Fleet Deployment</span>
                )}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
