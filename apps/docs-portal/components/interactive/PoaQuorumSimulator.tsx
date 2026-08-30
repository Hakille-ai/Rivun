'use client';

import React, { useState, useMemo } from 'react';
import { Cpu, ShieldCheck, ShieldAlert, AlertTriangle, RefreshCw } from 'lucide-react';
import { Badge } from '@/components/ui/Badge';

type NodeState = 'healthy' | 'offline' | 'byzantine';

interface ValidatorNode {
  id: number;
  name: string;
  state: NodeState;
}

export function PoaQuorumSimulator() {
  const [totalNodes, setTotalNodes] = useState(5);
  const [customThreshold, setCustomThreshold] = useState<number | null>(null);
  const [nodeStates, setNodeStates] = useState<Record<number, NodeState>>({
    1: 'healthy',
    2: 'healthy',
    3: 'healthy',
    4: 'healthy',
    5: 'healthy',
  });

  // Calculate default BFT threshold: floor(2N/3) + 1
  const bftThreshold = useMemo(() => {
    return Math.floor((2 * totalNodes) / 3) + 1;
  }, [totalNodes]);

  const maxByzantineFaults = useMemo(() => {
    return Math.floor((totalNodes - 1) / 3);
  }, [totalNodes]);

  const threshold = customThreshold !== null ? customThreshold : bftThreshold;

  // Count states
  const healthyCount = useMemo(() => {
    let count = 0;
    for (let i = 1; i <= totalNodes; i++) {
      if ((nodeStates[i] || 'healthy') === 'healthy') count++;
    }
    return count;
  }, [nodeStates, totalNodes]);

  const byzantineCount = useMemo(() => {
    let count = 0;
    for (let i = 1; i <= totalNodes; i++) {
      if (nodeStates[i] === 'byzantine') count++;
    }
    return count;
  }, [nodeStates, totalNodes]);

  const isQuorumReached = healthyCount >= threshold;
  const isByzantineCompromised = byzantineCount > maxByzantineFaults;

  const handleToggleNode = (id: number) => {
    setNodeStates((prev) => {
      const current = prev[id] || 'healthy';
      const next: NodeState =
        current === 'healthy' ? 'offline' : current === 'offline' ? 'byzantine' : 'healthy';
      return {
        ...prev,
        [id]: next,
      };
    });
  };

  const handleResetAll = () => {
    const fresh: Record<number, NodeState> = {};
    for (let i = 1; i <= totalNodes; i++) fresh[i] = 'healthy';
    setNodeStates(fresh);
  };

  return (
    <div className="space-y-6">
      <div className="p-6 rounded-2xl border border-border-subtle bg-bg-surface shadow-card">
        <div className="flex items-center justify-between pb-4 mb-5 border-b border-border-subtle">
          <div className="flex items-center gap-2">
            <Cpu className="w-5 h-5 text-accent-primary" />
            <h3 className="text-base font-bold text-text-primary">
              Proof-of-Action Quorum Calculator ($T \le N$)
            </h3>
          </div>
          <button
            onClick={handleResetAll}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-bg-subtle hover:bg-bg-surface-raised border border-border-subtle text-xs text-text-secondary hover:text-text-primary transition-colors"
          >
            <RefreshCw className="w-3.5 h-3.5" />
            <span>Reset Swarm</span>
          </button>
        </div>

        {/* Sliders & Configuration */}
        <div className="grid grid-cols-1 md:grid-cols-3 gap-6 mb-8">
          <div>
            <div className="flex justify-between text-xs font-semibold text-text-primary mb-2">
              <span>Total Validators (N)</span>
              <span className="font-mono text-cyan-400">{totalNodes} Nodes</span>
            </div>
            <input
              type="range"
              min={3}
              max={15}
              value={totalNodes}
              onChange={(e) => {
                const val = Number(e.target.value);
                setTotalNodes(val);
                setCustomThreshold(null);
              }}
              className="w-full accent-cyan-400 cursor-pointer"
            />
          </div>

          <div>
            <div className="flex justify-between text-xs font-semibold text-text-primary mb-2">
              <span>Quorum Threshold (T)</span>
              <span className="font-mono text-cyan-400">{threshold} of {totalNodes}</span>
            </div>
            <div className="flex items-center gap-2">
              <input
                type="range"
                min={1}
                max={totalNodes}
                value={threshold}
                onChange={(e) => setCustomThreshold(Number(e.target.value))}
                className="w-full accent-indigo-400 cursor-pointer"
              />
            </div>
          </div>

          <div className="p-3 rounded-xl bg-bg-subtle border border-border-subtle flex flex-col justify-center">
            <span className="text-[11px] text-text-muted">BFT Fault Bound:</span>
            <span className="text-xs font-mono font-semibold text-text-primary">
              Max Byzantine Faults $F = {maxByzantineFaults}$
            </span>
          </div>
        </div>

        {/* Quorum Status Alert Banner */}
        <div
          className={`p-4 rounded-xl border flex items-center justify-between transition-all ${
            isQuorumReached && !isByzantineCompromised
              ? 'bg-emerald-950/25 border-emerald-500/40 text-emerald-300'
              : 'bg-rose-950/25 border-rose-500/40 text-rose-300'
          }`}
        >
          <div className="flex items-center gap-3">
            {isQuorumReached && !isByzantineCompromised ? (
              <ShieldCheck className="w-6 h-6 text-emerald-400 shrink-0" />
            ) : (
              <ShieldAlert className="w-6 h-6 text-rose-400 shrink-0" />
            )}
            <div>
              <div className="text-sm font-bold">
                {isQuorumReached && !isByzantineCompromised
                  ? 'BFT Quorum Validated: Action Approved'
                  : isByzantineCompromised
                  ? 'Security Boundary Violated: Byzantine Attack Detected'
                  : 'Consensus Stalled: Insufficient Healthy Signatures'}
              </div>
              <div className="text-xs opacity-80 mt-0.5">
                Active signatures: {healthyCount}/{threshold} required ({totalNodes} total nodes in swarm).
              </div>
            </div>
          </div>
          <Badge
            variant={isQuorumReached && !isByzantineCompromised ? 'emerald' : 'rose'}
          >
            {isQuorumReached && !isByzantineCompromised ? 'QUORUM REACHED' : 'QUORUM FAILED'}
          </Badge>
        </div>

        {/* Interactive Node Grid */}
        <div className="mt-8">
          <div className="text-xs font-semibold text-text-primary uppercase tracking-wider mb-3">
            Validator Swarm Topology (Click nodes to cycle: Healthy → Offline → Byzantine)
          </div>
          <div className="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3">
            {Array.from({ length: totalNodes }, (_, i) => i + 1).map((nodeId) => {
              const state = nodeStates[nodeId] || 'healthy';
              return (
                <button
                  key={nodeId}
                  onClick={() => handleToggleNode(nodeId)}
                  className={`p-4 rounded-xl border text-left transition-all ${
                    state === 'healthy'
                      ? 'bg-emerald-950/20 border-emerald-500/40 hover:border-emerald-400 shadow-card'
                      : state === 'offline'
                      ? 'bg-bg-subtle border-border-subtle text-text-muted hover:border-border-strong'
                      : 'bg-rose-950/25 border-rose-500/50 hover:border-rose-400 shadow-card'
                  }`}
                >
                  <div className="flex items-center justify-between mb-2">
                    <span className="text-xs font-mono font-bold text-text-primary">
                      Node #{nodeId}
                    </span>
                    <span
                      className={`w-2 h-2 rounded-full ${
                        state === 'healthy'
                          ? 'bg-emerald-400 animate-pulse'
                          : state === 'offline'
                          ? 'bg-slate-600'
                          : 'bg-rose-400 animate-ping'
                      }`}
                    />
                  </div>
                  <div className="text-[11px] font-mono capitalize">
                    {state === 'healthy' && (
                      <span className="text-emerald-400 font-semibold">Healthy (Voting)</span>
                    )}
                    {state === 'offline' && (
                      <span className="text-slate-400">Offline (Timeout)</span>
                    )}
                    {state === 'byzantine' && (
                      <span className="text-rose-400 font-semibold">Byzantine (Equivocating)</span>
                    )}
                  </div>
                </button>
              );
            })}
          </div>
        </div>
      </div>
    </div>
  );
}
