"use client";

import React, { useRef, useEffect, useState, useCallback } from "react";
import {
  Layers,
  Radio,
  Zap,
  ShieldAlert,
  Sliders,
  Activity,
  CheckCircle2,
  AlertTriangle,
  RotateCcw,
  Sparkles,
} from "lucide-react";

interface SwarmNode {
  id: string;
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  role: "client" | "relay" | "validator" | "byzantine";
  state: "idle" | "gossip_active" | "propose" | "prevote" | "precommit" | "committed" | "partitioned";
  pulsePhase: number;
  color: string;
  uuid: string;
  isLeader?: boolean;
}

interface GossipParticle {
  fromX: number;
  fromY: number;
  toX: number;
  toY: number;
  currentX: number;
  currentY: number;
  progress: number;
  speed: number;
  color: string;
}

export function SwarmVisualizer() {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const [nodeCount, setNodeCount] = useState(24);
  const [isPartitioned, setIsPartitioned] = useState(false);
  const [activeConsensusStep, setActiveConsensusStep] = useState<string | null>(null);
  const [quorumProgress, setQuorumProgress] = useState<{ current: number; threshold: number; total: number } | null>(null);
  const [stats, setStats] = useState({
    activeNodes: 24,
    gossipLatency: "0.38 ms",
    meshLinks: 78,
    swarmHealth: 100,
    opsSec: 12450,
  });

  const nodesRef = useRef<SwarmNode[]>([]);
  const particlesRef = useRef<GossipParticle[]>([]);
  const animationFrameId = useRef<number | null>(null);

  // Initialize swarm nodes
  const initSwarm = useCallback((count: number) => {
    const nodes: SwarmNode[] = [];
    const width = 800;
    const height = 480;
    const centerX = width / 2;
    const centerY = height / 2;

    const validatorCount = Math.max(4, Math.floor(count * 0.3));

    for (let i = 0; i < count; i++) {
      const isValidator = i < validatorCount;
      const isByzantine = i === count - 1;
      const isRelay = !isValidator && !isByzantine && i % 2 === 0;
      const role: SwarmNode["role"] = isByzantine
        ? "byzantine"
        : isValidator
        ? "validator"
        : isRelay
        ? "relay"
        : "client";

      const angle = (i / count) * Math.PI * 2;
      const radiusDist = isValidator ? 110 + (Math.random() * 20 - 10) : 180 + (Math.random() * 40 - 20);
      const x = centerX + Math.cos(angle) * radiusDist;
      const y = centerY + Math.sin(angle) * radiusDist;

      let color = "#5B8CFF"; // relay
      if (role === "validator") color = "#F59E0B"; // gold
      if (role === "client") color = "#00F2FE"; // cyan
      if (role === "byzantine") color = "#F0554D"; // red

      nodes.push({
        id: `node-${i + 1}`,
        x,
        y,
        vx: (Math.random() - 0.5) * 0.4,
        vy: (Math.random() - 0.5) * 0.4,
        radius: isValidator ? 7 : 5,
        role,
        state: "idle",
        pulsePhase: Math.random() * Math.PI * 2,
        color,
        uuid: `8f2a41d9-${(i + 10).toString(16).padStart(4, "0")}-4000-8000-000000000000`,
        isLeader: i === 0,
      });
    }
    nodesRef.current = nodes;
    particlesRef.current = [];
  }, []);

  useEffect(() => {
    initSwarm(nodeCount);
    setStats((prev) => ({
      ...prev,
      activeNodes: nodeCount,
      meshLinks: Math.floor(nodeCount * 3.2),
      swarmHealth: isPartitioned ? 68 : 100,
    }));
  }, [nodeCount, initSwarm, isPartitioned]);

  // Trigger Gossip Ripple
  const triggerGossipWave = () => {
    const nodes = nodesRef.current;
    if (nodes.length === 0) return;

    // Pick source node (or leader)
    const source = nodes[0];
    source.state = "gossip_active";

    // Cascade particles to k=3 nearest neighbors
    const newParticles: GossipParticle[] = [];
    const sorted = [...nodes].sort((a, b) => {
      const distA = Math.hypot(a.x - source.x, a.y - source.y);
      const distB = Math.hypot(b.x - source.x, b.y - source.y);
      return distA - distB;
    });

    for (let i = 1; i <= Math.min(4, sorted.length - 1); i++) {
      const target = sorted[i];
      if (isPartitioned && target.x > 400 && source.x <= 400) continue;
      newParticles.push({
        fromX: source.x,
        fromY: source.y,
        toX: target.x,
        toY: target.y,
        currentX: source.x,
        currentY: source.y,
        progress: 0,
        speed: 0.04 + Math.random() * 0.02,
        color: "#5B8CFF",
      });
    }

    particlesRef.current.push(...newParticles);

    // Secondary cascade after 200ms
    setTimeout(() => {
      for (let i = 1; i <= 4; i++) {
        const relayNode = sorted[i];
        if (!relayNode) continue;
        relayNode.state = "gossip_active";

        for (let j = 5; j < Math.min(12, sorted.length); j++) {
          const leaf = sorted[j];
          if (isPartitioned && leaf.x > 400 && relayNode.x <= 400) continue;
          particlesRef.current.push({
            fromX: relayNode.x,
            fromY: relayNode.y,
            toX: leaf.x,
            toY: leaf.y,
            currentX: relayNode.x,
            currentY: relayNode.y,
            progress: 0,
            speed: 0.05 + Math.random() * 0.02,
            color: "#3DD68C",
          });
        }
      }
    }, 200);

    // Reset states after 1.5s
    setTimeout(() => {
      nodes.forEach((n) => (n.state = "idle"));
    }, 1500);
  };

  // Trigger 2-Phase BFT Quorum
  const triggerBftQuorum = () => {
    const nodes = nodesRef.current;
    const validators = nodes.filter((n) => n.role === "validator");
    const N = validators.length;
    const T = Math.floor((2 * N) / 3) + 1;

    // Step 1: PROPOSE (Leader pulse)
    setActiveConsensusStep("PROPOSE");
    setQuorumProgress({ current: 1, threshold: T, total: N });
    validators[0].state = "propose";

    // Broadcast proposal particles to all validators
    for (let i = 1; i < validators.length; i++) {
      particlesRef.current.push({
        fromX: validators[0].x,
        fromY: validators[0].y,
        toX: validators[i].x,
        toY: validators[i].y,
        currentX: validators[0].x,
        currentY: validators[0].y,
        progress: 0,
        speed: 0.05,
        color: "#F59E0B", // Gold proposal
      });
    }

    // Step 2: PREVOTE
    setTimeout(() => {
      setActiveConsensusStep("PREVOTE");
      setQuorumProgress({ current: Math.min(T, N), threshold: T, total: N });
      validators.forEach((v, idx) => {
        if (idx !== N - 1) v.state = "prevote"; // all except byzantine prevote
      });
    }, 450);

    // Step 3: PRECOMMIT
    setTimeout(() => {
      setActiveConsensusStep("PRECOMMIT");
      setQuorumProgress({ current: T, threshold: T, total: N });
      validators.forEach((v, idx) => {
        if (idx !== N - 1) v.state = "precommit";
      });
    }, 900);

    // Step 4: COMMIT CERTIFICATE
    setTimeout(() => {
      setActiveConsensusStep("COMMIT CERTIFICATE (QUORUM SEALED)");
      setQuorumProgress({ current: T, threshold: T, total: N });
      validators.forEach((v) => (v.state = "committed"));
    }, 1350);

    // Reset
    setTimeout(() => {
      setActiveConsensusStep(null);
      setQuorumProgress(null);
      nodes.forEach((n) => (n.state = "idle"));
    }, 2800);
  };

  // Chaos partition toggle
  const togglePartition = () => {
    setIsPartitioned((p) => {
      const next = !p;
      setStats((s) => ({
        ...s,
        swarmHealth: next ? 68 : 100,
        gossipLatency: next ? "1.45 ms (2-Hop Failover)" : "0.38 ms",
      }));
      return next;
    });
  };

  // Main Canvas Render Loop
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    let isRunning = true;

    const render = () => {
      if (!isRunning) return;

      const width = canvas.width;
      const height = canvas.height;
      const centerX = width / 2;
      const centerY = height / 2;

      ctx.clearRect(0, 0, width, height);

      // 1. Draw Mesh Background Rings
      ctx.strokeStyle = "rgba(255, 255, 255, 0.04)";
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.arc(centerX, centerY, 110, 0, Math.PI * 2);
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(centerX, centerY, 180, 0, Math.PI * 2);
      ctx.stroke();

      // 2. Draw Partition Wall if active
      if (isPartitioned) {
        ctx.strokeStyle = "rgba(240, 85, 77, 0.4)";
        ctx.setLineDash([6, 6]);
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(centerX, 20);
        ctx.lineTo(centerX, height - 20);
        ctx.stroke();
        ctx.setLineDash([]);

        // Label
        ctx.fillStyle = "#F0554D";
        ctx.font = "10px JetBrains Mono";
        ctx.fillText("NETWORK PARTITION BARRIER (SPLIT-BRAIN ISOLATION)", centerX - 130, 30);
      }

      const nodes = nodesRef.current;

      // 3. Draw Spring Mesh Links between nearby nodes
      ctx.lineWidth = 0.75;
      for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
          const a = nodes[i];
          const b = nodes[j];

          // Skip link if partitioned across center
          if (isPartitioned && ((a.x < centerX && b.x > centerX) || (a.x > centerX && b.x < centerX))) {
            continue;
          }

          const dist = Math.hypot(a.x - b.x, a.y - b.y);
          if (dist < 110) {
            const alpha = (1 - dist / 110) * 0.25;
            ctx.strokeStyle = `rgba(91, 140, 255, ${alpha})`;
            ctx.beginPath();
            ctx.moveTo(a.x, a.y);
            ctx.lineTo(b.x, b.y);
            ctx.stroke();
          }
        }
      }

      // 4. Update and Draw Gossip / BFT Particles
      const activeParticles = particlesRef.current;
      for (let pIdx = activeParticles.length - 1; pIdx >= 0; pIdx--) {
        const p = activeParticles[pIdx];
        p.progress += p.speed;
        p.currentX = p.fromX + (p.toX - p.fromX) * p.progress;
        p.currentY = p.fromY + (p.toY - p.fromY) * p.progress;

        ctx.fillStyle = p.color;
        ctx.shadowColor = p.color;
        ctx.shadowBlur = 8;
        ctx.beginPath();
        ctx.arc(p.currentX, p.currentY, 2.5, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;

        if (p.progress >= 1) {
          activeParticles.splice(pIdx, 1);
        }
      }

      // 5. Update and Draw Swarm Nodes
      for (let i = 0; i < nodes.length; i++) {
        const n = nodes[i];

        // Physics Drift
        n.x += n.vx;
        n.y += n.vy;
        n.pulsePhase += 0.05;

        // Boundaries & center gravity
        const distFromCenter = Math.hypot(n.x - centerX, n.y - centerY);
        if (distFromCenter > 220) {
          n.vx -= (n.x - centerX) * 0.001;
          n.vy -= (n.y - centerY) * 0.001;
        }

        // Draw State Halo
        if (n.state !== "idle") {
          ctx.beginPath();
          let haloColor = "rgba(91, 140, 255, 0.3)";
          if (n.state === "propose") haloColor = "rgba(245, 158, 11, 0.4)";
          if (n.state === "prevote") haloColor = "rgba(0, 242, 254, 0.4)";
          if (n.state === "precommit") haloColor = "rgba(61, 214, 140, 0.4)";
          if (n.state === "committed") haloColor = "rgba(61, 214, 140, 0.6)";

          ctx.fillStyle = haloColor;
          const pulseR = n.radius + 6 + Math.sin(n.pulsePhase) * 3;
          ctx.arc(n.x, n.y, pulseR, 0, Math.PI * 2);
          ctx.fill();
        }

        // Draw Node Core
        ctx.fillStyle = n.color;
        ctx.shadowColor = n.color;
        ctx.shadowBlur = n.role === "validator" ? 10 : 4;
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
        ctx.fill();
        ctx.shadowBlur = 0;

        // Inner core
        ctx.fillStyle = "#0A0B0D";
        ctx.beginPath();
        ctx.arc(n.x, n.y, n.radius * 0.4, 0, Math.PI * 2);
        ctx.fill();
      }

      animationFrameId.current = requestAnimationFrame(render);
    };

    render();

    return () => {
      isRunning = false;
      if (animationFrameId.current) {
        cancelAnimationFrame(animationFrameId.current);
      }
    };
  }, [isPartitioned]);

  return (
    <section id="swarm" className="py-24 relative overflow-hidden">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Section Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <Layers className="w-3.5 h-3.5" />
            <span>P2P SWARM & CONSENSUS FABRIC</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Epidemic Gossip Waves & 2-Phase BFT Quorum Mesh
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            High-frequency message diffusion with $k$-fanout epidemic gossip, vector clocks for causal ordering,
            and deterministic BFT Proof-of-Action quorum ($T \le N$) with instant Byzantine equivocation slashing.
          </p>
        </div>

        {/* Visualizer Frame */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 shadow-2xl relative overflow-hidden">
          {/* Top Control HUD */}
          <div className="flex flex-wrap items-center justify-between gap-4 pb-6 border-b border-[#22262F]">
            {/* Interactive Triggers */}
            <div className="flex flex-wrap items-center gap-2.5">
              <button
                onClick={triggerGossipWave}
                className="px-4 py-2 text-xs font-semibold text-white bg-[#5B8CFF] hover:bg-[#4378F0] rounded-xl shadow-glow transition-all flex items-center gap-2"
              >
                <Radio className="w-3.5 h-3.5" />
                <span>Broadcast Gossip Wave</span>
              </button>

              <button
                onClick={triggerBftQuorum}
                className="px-4 py-2 text-xs font-semibold text-white bg-amber-600 hover:bg-amber-500 rounded-xl transition-all flex items-center gap-2"
              >
                <Zap className="w-3.5 h-3.5" />
                <span>Execute BFT PoA Round</span>
              </button>

              <button
                onClick={togglePartition}
                className={`px-4 py-2 text-xs font-semibold rounded-xl border transition-all flex items-center gap-2 ${
                  isPartitioned
                    ? "bg-rose-500/20 text-rose-400 border-rose-500/50"
                    : "bg-[#181B22] text-[#9AA1AE] border-[#22262F] hover:text-white"
                }`}
              >
                <ShieldAlert className="w-3.5 h-3.5" />
                <span>{isPartitioned ? "Heal Network Partition" : "Simulate Partition Chaos"}</span>
              </button>
            </div>

            {/* Node Count Slider */}
            <div className="flex items-center gap-3 bg-[#181B22] px-4 py-2 rounded-xl border border-[#22262F]">
              <Sliders className="w-3.5 h-3.5 text-[#5B8CFF]" />
              <span className="text-xs text-[#9AA1AE]">Nodes:</span>
              <input
                type="range"
                min={12}
                max={48}
                step={4}
                value={nodeCount}
                onChange={(e) => setNodeCount(Number(e.target.value))}
                className="w-24 accent-[#5B8CFF] cursor-pointer"
              />
              <span className="text-xs font-mono font-bold text-white w-6 text-right">{nodeCount}</span>
            </div>
          </div>

          {/* Canvas Viewport */}
          <div className="relative w-full h-[480px] my-4 rounded-xl bg-[#0A0B0D] border border-[#22262F] flex items-center justify-center overflow-hidden">
            <canvas
              ref={canvasRef}
              width={800}
              height={480}
              className="w-full h-full max-w-[800px] max-h-[480px]"
            />

            {/* Live BFT Status Overlay */}
            {activeConsensusStep && (
              <div className="absolute top-4 left-4 bg-[#111318]/90 border border-[#F59E0B]/40 rounded-xl p-3 backdrop-blur-md shadow-2xl animate-fade-in">
                <div className="flex items-center gap-2 mb-1">
                  <span className="w-2 h-2 rounded-full bg-[#F59E0B] animate-ping" />
                  <span className="text-xs font-mono font-bold text-[#F59E0B]">
                    CONSENSUS STEP: {activeConsensusStep}
                  </span>
                </div>
                {quorumProgress && (
                  <div className="text-[11px] text-[#9AA1AE] font-mono">
                    Attestations: {quorumProgress.current}/{quorumProgress.total} (Quorum Threshold T = {quorumProgress.threshold})
                  </div>
                )}
              </div>
            )}

            {/* Topology Legend */}
            <div className="absolute bottom-4 right-4 bg-[#111318]/90 border border-[#22262F] rounded-xl px-3 py-2 text-[10px] font-mono flex items-center gap-3 backdrop-blur-md">
              <span className="flex items-center gap-1 text-[#00F2FE]">
                <span className="w-2 h-2 rounded-full bg-[#00F2FE]" /> Edge Client
              </span>
              <span className="flex items-center gap-1 text-[#5B8CFF]">
                <span className="w-2 h-2 rounded-full bg-[#5B8CFF]" /> Swarm Relay
              </span>
              <span className="flex items-center gap-1 text-[#F59E0B]">
                <span className="w-2 h-2 rounded-full bg-[#F59E0B]" /> BFT Validator
              </span>
              <span className="flex items-center gap-1 text-[#F0554D]">
                <span className="w-2 h-2 rounded-full bg-[#F0554D]" /> Slashed Byzantine
              </span>
            </div>
          </div>

          {/* Telemetry HUD Bottom Bar */}
          <div className="grid grid-cols-2 sm:grid-cols-5 gap-3 pt-4 border-t border-[#22262F] text-xs font-mono">
            <div className="p-3 bg-[#14171F] rounded-xl border border-[#22262F]">
              <span className="text-[#6B7280] block text-[10px] uppercase">Active Mesh Nodes</span>
              <span className="text-white font-bold text-sm">{stats.activeNodes} Active</span>
            </div>
            <div className="p-3 bg-[#14171F] rounded-xl border border-[#22262F]">
              <span className="text-[#6B7280] block text-[10px] uppercase">Gossip Latency</span>
              <span className="text-[#3DD68C] font-bold text-sm">{stats.gossipLatency}</span>
            </div>
            <div className="p-3 bg-[#14171F] rounded-xl border border-[#22262F]">
              <span className="text-[#6B7280] block text-[10px] uppercase">Active Mesh Links</span>
              <span className="text-[#5B8CFF] font-bold text-sm">{stats.meshLinks} Links</span>
            </div>
            <div className="p-3 bg-[#14171F] rounded-xl border border-[#22262F]">
              <span className="text-[#6B7280] block text-[10px] uppercase">Swarm Health</span>
              <span className={`font-bold text-sm ${stats.swarmHealth === 100 ? "text-[#3DD68C]" : "text-amber-400"}`}>
                {stats.swarmHealth}%
              </span>
            </div>
            <div className="p-3 bg-[#14171F] rounded-xl border border-[#22262F] col-span-2 sm:col-span-1">
              <span className="text-[#6B7280] block text-[10px] uppercase">Consensus Throughput</span>
              <span className="text-white font-bold text-sm">{stats.opsSec.toLocaleString()} ops/s</span>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
