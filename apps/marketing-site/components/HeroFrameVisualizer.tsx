"use client";

import React, { useState, useMemo } from "react";
import {
  Code,
  ShieldCheck,
  Zap,
  Layers,
  Cpu,
  Lock,
  Radio,
  FileCode,
  CheckCircle2,
  Copy,
  Check,
  ChevronDown,
  Info,
  Hash,
  Binary,
} from "lucide-react";
import { MessageKindName, ProtocolFlags } from "../lib/types";
import { encodeRivunFrame } from "../lib/protocol";

const PRESET_PAYLOADS: Record<MessageKindName, { subject: string; contentType: string; payload: string }> = {
  data: {
    subject: "sensor.temperature.telemetry",
    contentType: "application/json",
    payload: JSON.stringify({ celsius: 21.84, humidity: 45.2, sensor_id: "edge-temp-node-04" }, null, 2),
  },
  event: {
    subject: "cluster.node.joined",
    contentType: "application/json",
    payload: JSON.stringify({ node_id: "node-bravo-7788", region: "eu-west-1", roles: ["validator", "relay"] }, null, 2),
  },
  command: {
    subject: "plc.valve.actuate",
    contentType: "application/json",
    payload: JSON.stringify({ valve_id: 12, target_state: "OPEN", safety_override_token: "tok_0x9923a1" }, null, 2),
  },
  query: {
    subject: "ledger.receipt.query",
    contentType: "application/json",
    payload: JSON.stringify({ receipt_hash: "blake3:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855" }, null, 2),
  },
  response: {
    subject: "ledger.receipt.status",
    contentType: "application/json",
    payload: JSON.stringify({ status: "VERIFIED_ON_CHAIN", mmr_peak_height: 48, confirmed_round: 10452 }, null, 2),
  },
  streamChunk: {
    subject: "camera.stream.frame",
    contentType: "application/octet-stream",
    payload: JSON.stringify({ frame_seq: 14209, chunk_index: 4, chunk_total: 16, ring_offset_bytes: 4096 }, null, 2),
  },
  action: {
    subject: "agent.code.patch",
    contentType: "application/rivun-agent+json",
    payload: JSON.stringify({ action: "repo.patch", target_file: "src/auth/vault.rs", diff_hash: "blake3:8f2a41d9..." }, null, 2),
  },
  control: {
    subject: "bft.consensus.vote",
    contentType: "application/rivun-control+json",
    payload: JSON.stringify({ epoch: 142, round: 1, step: "PRECOMMIT", proposal_digest: "blake3:44d9f1..." }, null, 2),
  },
};

export function HeroFrameVisualizer() {
  const [kind, setKind] = useState<MessageKindName>("action");
  const [subject, setSubject] = useState("agent.code.patch");
  const [contentType, setContentType] = useState("application/rivun-agent+json");
  const [payloadJson, setPayloadJson] = useState(
    JSON.stringify({ action: "repo.patch", target_file: "src/auth/vault.rs", diff_hash: "blake3:8f2a41d9..." }, null, 2)
  );
  const [flags, setFlags] = useState<ProtocolFlags>({
    signed: true,
    requiresConsensus: true,
    encrypted: false,
    priority: false,
    broadcast: false,
  });
  const [activeView, setActiveView] = useState<"tree" | "hexdump">("tree");
  const [hoveredSegment, setHoveredSegment] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);

  const handleSelectPreset = (newKind: MessageKindName) => {
    setKind(newKind);
    const preset = PRESET_PAYLOADS[newKind];
    setSubject(preset.subject);
    setContentType(preset.contentType);
    setPayloadJson(preset.payload);
  };

  const encodedFrame = useMemo(() => {
    try {
      return encodeRivunFrame({
        kind,
        subject,
        contentType,
        payloadJson,
        flags,
        poaThreshold: 2,
        poaAttestationCount: 3,
      });
    } catch {
      return encodeRivunFrame({
        kind: "data",
        subject: "error.fallback",
        contentType: "application/json",
        payloadJson: "{}",
        flags,
      });
    }
  }, [kind, subject, contentType, payloadJson, flags]);

  const copyHex = () => {
    const hex = Array.from(encodedFrame.rawBytes)
      .map((b) => b.toString(16).padStart(2, "0"))
      .join("");
    navigator.clipboard.writeText(hex);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="w-full bg-[#111318]/90 backdrop-blur-2xl border border-[#22262F] rounded-2xl p-4 sm:p-6 lg:p-8 shadow-2xl relative overflow-hidden group">
      {/* Background ambient light */}
      <div className="absolute -top-32 -right-32 w-80 h-80 bg-[#5B8CFF]/10 rounded-full blur-3xl pointer-events-none" />
      <div className="absolute -bottom-32 -left-32 w-80 h-80 bg-[#3DD68C]/10 rounded-full blur-3xl pointer-events-none" />

      {/* Header Bar */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4 pb-6 border-b border-[#22262F]">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-[#5B8CFF]/15 border border-[#5B8CFF]/30 text-[#5B8CFF]">
            <Binary className="w-5 h-5" />
          </div>
          <div>
            <div className="flex items-center gap-2">
              <h3 className="text-base sm:text-lg font-bold text-white tracking-tight">
                Live Binary Wire Frame Encoder
              </h3>
              <span className="px-2 py-0.5 text-[10px] font-mono font-bold bg-[#3DD68C]/15 text-[#3DD68C] border border-[#3DD68C]/30 rounded-full flex items-center gap-1">
                <span className="w-1.5 h-1.5 rounded-full bg-[#3DD68C] animate-pulse" />
                64B HEADER + ZENV
              </span>
            </div>
            <p className="text-xs text-[#9AA1AE]">
              Bit-level zero-copy framing, Ed25519 authentication, and Proof-of-Action consensus trailer
            </p>
          </div>
        </div>

        {/* View Switcher & Copy */}
        <div className="flex items-center gap-2">
          <div className="bg-[#181B22] p-1 rounded-xl border border-[#22262F] flex items-center gap-1">
            <button
              onClick={() => setActiveView("tree")}
              className={`px-3 py-1.5 text-xs font-semibold rounded-lg transition-all flex items-center gap-1.5 ${
                activeView === "tree"
                  ? "bg-[#5B8CFF] text-white shadow-sm"
                  : "text-[#9AA1AE] hover:text-white"
              }`}
            >
              <Layers className="w-3.5 h-3.5" />
              <span>Byte-Tree</span>
            </button>
            <button
              onClick={() => setActiveView("hexdump")}
              className={`px-3 py-1.5 text-xs font-semibold rounded-lg transition-all flex items-center gap-1.5 ${
                activeView === "hexdump"
                  ? "bg-[#5B8CFF] text-white shadow-sm"
                  : "text-[#9AA1AE] hover:text-white"
              }`}
            >
              <Code className="w-3.5 h-3.5" />
              <span>Hex Dump</span>
            </button>
          </div>

          <button
            onClick={copyHex}
            className="p-2 rounded-xl bg-[#181B22] hover:bg-[#22262F] border border-[#22262F] text-[#9AA1AE] hover:text-white transition-all text-xs flex items-center gap-1.5"
            title="Copy Raw Hex Buffer"
          >
            {copied ? <Check className="w-4 h-4 text-[#3DD68C]" /> : <Copy className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Interactive Controls Panel */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6 py-6 border-b border-[#22262F]">
        {/* Left: Kind Selector & Flags (5 cols) */}
        <div className="lg:col-span-5 space-y-4">
          <div>
            <label className="text-xs font-semibold text-[#9AA1AE] uppercase tracking-wider block mb-2">
              1. Select Protocol Message Kind
            </label>
            <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
              {(Object.keys(PRESET_PAYLOADS) as MessageKindName[]).map((k) => (
                <button
                  key={k}
                  onClick={() => handleSelectPreset(k)}
                  className={`px-2.5 py-2 text-xs font-medium rounded-lg border transition-all text-left flex flex-col capitalize ${
                    kind === k
                      ? "bg-[#5B8CFF]/20 text-[#5B8CFF] border-[#5B8CFF]/50 shadow-sm"
                      : "bg-[#181B22] text-[#9AA1AE] border-[#22262F] hover:border-[#3A4150] hover:text-white"
                  }`}
                >
                  <span className="font-semibold">{k}</span>
                  <span className="text-[10px] text-[#6B7280] font-mono">
                    0x{PRESET_PAYLOADS[k] ? Object.keys(PRESET_PAYLOADS).indexOf(k) + 1 : 1}
                  </span>
                </button>
              ))}
            </div>
          </div>

          {/* Flags Toggles */}
          <div>
            <label className="text-xs font-semibold text-[#9AA1AE] uppercase tracking-wider block mb-2">
              2. Wire Security & Routing Flags
            </label>
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
              <button
                onClick={() => setFlags((f) => ({ ...f, signed: !f.signed }))}
                className={`px-3 py-2 text-xs font-medium rounded-lg border transition-all flex items-center gap-2 ${
                  flags.signed
                    ? "bg-[#3DD68C]/15 text-[#3DD68C] border-[#3DD68C]/40"
                    : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                }`}
              >
                <ShieldCheck className="w-3.5 h-3.5" />
                <span>SIGNED (ZSIG)</span>
              </button>

              <button
                onClick={() => setFlags((f) => ({ ...f, requiresConsensus: !f.requiresConsensus }))}
                className={`px-3 py-2 text-xs font-medium rounded-lg border transition-all flex items-center gap-2 ${
                  flags.requiresConsensus
                    ? "bg-amber-400/15 text-amber-400 border-amber-400/40"
                    : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                }`}
              >
                <Zap className="w-3.5 h-3.5" />
                <span>POA QUORUM</span>
              </button>

              <button
                onClick={() => setFlags((f) => ({ ...f, encrypted: !f.encrypted }))}
                className={`px-3 py-2 text-xs font-medium rounded-lg border transition-all flex items-center gap-2 ${
                  flags.encrypted
                    ? "bg-purple-400/15 text-purple-400 border-purple-400/40"
                    : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                }`}
              >
                <Lock className="w-3.5 h-3.5" />
                <span>ENCRYPTED</span>
              </button>

              <button
                onClick={() => setFlags((f) => ({ ...f, priority: !f.priority }))}
                className={`px-3 py-2 text-xs font-medium rounded-lg border transition-all flex items-center gap-2 ${
                  flags.priority
                    ? "bg-rose-400/15 text-rose-400 border-rose-400/40"
                    : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                }`}
              >
                <Radio className="w-3.5 h-3.5" />
                <span>PRIORITY</span>
              </button>

              <button
                onClick={() => setFlags((f) => ({ ...f, broadcast: !f.broadcast }))}
                className={`px-3 py-2 text-xs font-medium rounded-lg border transition-all flex items-center gap-2 ${
                  flags.broadcast
                    ? "bg-cyan-400/15 text-cyan-400 border-cyan-400/40"
                    : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                }`}
              >
                <Cpu className="w-3.5 h-3.5" />
                <span>BROADCAST</span>
              </button>
            </div>
          </div>
        </div>

        {/* Right: Subject, Content-Type, and Payload Editor (7 cols) */}
        <div className="lg:col-span-7 space-y-3">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                Subject (Routing Key)
              </label>
              <input
                type="text"
                value={subject}
                onChange={(e) => setSubject(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] focus:border-[#5B8CFF] rounded-lg text-white outline-none transition-colors"
                placeholder="domain.subsystem.action"
              />
            </div>
            <div>
              <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                Content-Type
              </label>
              <input
                type="text"
                value={contentType}
                onChange={(e) => setContentType(e.target.value)}
                className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] focus:border-[#5B8CFF] rounded-lg text-white outline-none transition-colors"
                placeholder="application/json"
              />
            </div>
          </div>

          <div>
            <div className="flex items-center justify-between mb-1">
              <label className="text-xs font-semibold text-[#9AA1AE]">
                ZENV Body Payload (JSON or Raw Bytes)
              </label>
              <span className="text-[10px] font-mono text-[#6B7280]">
                {encodedFrame.payloadSize} payload bytes
              </span>
            </div>
            <textarea
              rows={3}
              value={payloadJson}
              onChange={(e) => setPayloadJson(e.target.value)}
              className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] focus:border-[#5B8CFF] rounded-lg text-emerald-400 outline-none transition-colors resize-none"
            />
          </div>
        </div>
      </div>

      {/* Frame Metrics Strip */}
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-3 py-4 text-xs font-mono border-b border-[#22262F] bg-[#14171F]/40 -mx-4 px-4 sm:-mx-6 sm:px-6 lg:-mx-8 lg:px-8">
        <div>
          <span className="text-[#6B7280] block text-[10px] uppercase">Total Encoded Frame</span>
          <span className="text-white font-bold text-sm">{encodedFrame.totalSize} Bytes</span>
        </div>
        <div>
          <span className="text-[#6B7280] block text-[10px] uppercase">BLAKE3 Frame Digest</span>
          <span className="text-[#5B8CFF] font-medium truncate block" title={encodedFrame.blake3Digest}>
            {encodedFrame.blake3Digest.slice(0, 14)}...
          </span>
        </div>
        <div>
          <span className="text-[#6B7280] block text-[10px] uppercase">Fast-Rejection Hint</span>
          <span className="text-rose-400 font-bold">0x{encodedFrame.signatureHint}</span>
        </div>
        <div>
          <span className="text-[#6B7280] block text-[10px] uppercase">Consensus Mode</span>
          <span className="text-amber-400 font-semibold">
            {flags.requiresConsensus ? "BFT Quorum (T=2, K=3)" : "Unilateral Signed"}
          </span>
        </div>
      </div>

      {/* Dynamic Inspector Content */}
      <div className="pt-6">
        {activeView === "tree" ? (
          /* Annotated Byte-Tree View */
          <div className="space-y-3">
            <div className="flex items-center justify-between text-xs text-[#9AA1AE] pb-2">
              <span className="font-semibold uppercase tracking-wider">
                Structured Binary Segment Breakdown ({encodedFrame.segments.length} Fields)
              </span>
              <span className="text-[11px] text-[#6B7280]">Hover any block to inspect exact byte offsets</span>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
              {encodedFrame.segments.map((seg, idx) => (
                <div
                  key={idx}
                  onMouseEnter={() => setHoveredSegment(idx)}
                  onMouseLeave={() => setHoveredSegment(null)}
                  className={`p-3.5 rounded-xl border transition-all ${
                    hoveredSegment === idx
                      ? "bg-[#181B22] border-[#5B8CFF] shadow-glow scale-[1.02]"
                      : "bg-[#14171F] border-[#22262F] hover:border-[#2E3440]"
                  }`}
                >
                  <div className="flex items-center justify-between mb-1.5">
                    <span className="font-mono text-[11px] font-bold text-white truncate mr-2">
                      {seg.name}
                    </span>
                    <span className="font-mono text-[10px] px-1.5 py-0.5 rounded bg-[#111318] text-[#9AA1AE] border border-[#22262F]">
                      [{seg.offset}..{seg.offset + seg.length}] ({seg.length}B)
                    </span>
                  </div>

                  <div className="font-mono text-xs text-[#5B8CFF] bg-[#0A0B0D] px-2 py-1.5 rounded-lg border border-[#22262F] truncate mb-2">
                    {seg.hex.length > 28 ? `${seg.hex.slice(0, 28)}...` : seg.hex}
                  </div>

                  <div className="text-[11px] text-[#9AA1AE] flex flex-col gap-0.5">
                    <span className="text-[#F4F5F7] font-medium truncate">{seg.parsedValue}</span>
                    <span className="text-[#6B7280] text-[10px] leading-relaxed">{seg.description}</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : (
          /* Live Hex Dump View */
          <div className="bg-[#0A0B0D] rounded-xl border border-[#22262F] p-4 font-mono text-xs overflow-x-auto shadow-inner">
            <div className="text-[11px] text-[#6B7280] border-b border-[#22262F] pb-2 mb-3 flex items-center justify-between">
              <div className="flex gap-4">
                <span>OFFSET</span>
                <span className="tracking-widest">00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F</span>
              </div>
              <span>ASCII DECODE</span>
            </div>

            <div className="space-y-1">
              {encodedFrame.hexDumpLines.map((line) => (
                <div
                  key={line.offset}
                  className="flex items-center justify-between hover:bg-[#181B22]/60 px-1 py-0.5 rounded transition-colors group/line"
                >
                  <div className="flex items-center gap-4">
                    <span className="text-[#6B7280] select-none">{line.offsetHex}:</span>
                    <div className="flex items-center gap-1.5">
                      {line.hexBytes.map((b, bIdx) => (
                        <React.Fragment key={bIdx}>
                          {bIdx === 8 && <span className="w-1 text-[#3A4150]"> </span>}
                          <span
                            onMouseEnter={() => setHoveredSegment(b.segmentIndex)}
                            onMouseLeave={() => setHoveredSegment(null)}
                            className={`hex-byte cursor-pointer px-0.5 rounded font-mono ${
                              hoveredSegment === b.segmentIndex
                                ? "bg-[#5B8CFF] text-white font-bold shadow-glow"
                                : `${b.colorClass} hover:text-white`
                            }`}
                            title={`Offset: ${b.globalOffset} (0x${b.globalOffset.toString(16)})`}
                          >
                            {b.byteHex}
                          </span>
                        </React.Fragment>
                      ))}
                    </div>
                  </div>
                  <span className="text-[#6B7280] font-mono tracking-wider select-none text-[11px]">
                    |{line.ascii}|
                  </span>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
