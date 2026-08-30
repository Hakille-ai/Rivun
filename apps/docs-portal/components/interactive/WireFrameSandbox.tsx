'use client';

import React, { useState, useMemo } from 'react';
import { Terminal, Copy, Check, RefreshCw, Cpu, Layers } from 'lucide-react';
import { Badge } from '@/components/ui/Badge';

export function WireFrameSandbox() {
  const [flags, setFlags] = useState<{
    encrypted: boolean;
    priority: boolean;
    requiresConsensus: boolean;
    signed: boolean;
    broadcast: boolean;
  }>({
    encrypted: false,
    priority: true,
    requiresConsensus: true,
    signed: true,
    broadcast: false,
  });

  const [sourceNode, setSourceNode] = useState('d8f1e09a-4c22-4819-bf91-30912384a101');
  const [targetNode, setTargetNode] = useState('a13c907b-8910-412e-9d21-998811223344');
  const [messageKind, setMessageKind] = useState('7'); // Action
  const [subject, setSubject] = useState('scada.hvac.temperature.set');
  const [payloadText, setPayloadText] = useState('{"zone": "datacenter-1", "target_c": 21.5}');
  const [copied, setCopied] = useState(false);

  // Compute Flag Mask
  const flagValue = useMemo(() => {
    let val = 0;
    if (flags.encrypted) val |= 0x0001;
    if (flags.priority) val |= 0x0002;
    if (flags.requiresConsensus) val |= 0x0004;
    if (flags.signed) val |= 0x0008;
    if (flags.broadcast) val |= 0x0010;
    return val;
  }, [flags]);

  // Compute Wire Representation
  const frameAnalysis = useMemo(() => {
    const payloadBytes = new TextEncoder().encode(payloadText);
    const payloadLen = payloadBytes.length;
    const headerLen = 64;
    const zsigLen = flags.signed ? 72 : 0;
    const zpoaLen = flags.requiresConsensus ? 120 : 0; // 40 + 80*1
    const totalLen = headerLen + payloadLen + zsigLen + zpoaLen;

    // Simulated Hex preview
    const magicHex = '5A 41 50 5F'; // "ZAP_"
    const versionHex = '00 01';
    const flagsHex = flagValue.toString(16).padStart(4, '0').match(/../g)?.join(' ').toUpperCase() || '00 00';
    const lenHex = payloadLen.toString(16).padStart(16, '0').match(/../g)?.join(' ').toUpperCase() || '';
    const signHintHex = 'E7 8A 1C 4D 9F 02 3B AA';

    return {
      payloadLen,
      headerLen,
      zsigLen,
      zpoaLen,
      totalLen,
      magicHex,
      versionHex,
      flagsHex,
      lenHex,
      signHintHex,
    };
  }, [flagValue, flags, payloadText]);

  const handleCopyHex = async () => {
    const hex = `5A41505F0001${flagValue.toString(16).padStart(4, '0')}...`;
    try {
      await navigator.clipboard.writeText(hex);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // ignore
    }
  };

  return (
    <div className="space-y-6">
      {/* Interactive Controls Panel */}
      <div className="p-6 rounded-2xl border border-border-subtle bg-bg-surface shadow-card">
        <div className="flex items-center justify-between pb-4 mb-5 border-b border-border-subtle">
          <div className="flex items-center gap-2">
            <Terminal className="w-5 h-5 text-accent-primary" />
            <h3 className="text-base font-bold text-text-primary">
              Interactive Wire Frame Builder
            </h3>
          </div>
          <Badge variant="cyan">Protocol v1 (64-byte Header)</Badge>
        </div>

        {/* Flag Bitfield Toggles */}
        <div className="space-y-3 mb-6">
          <label className="text-xs font-semibold text-text-primary uppercase tracking-wider block">
            RivunFlags (Bitfield 0x06 - 0x07)
          </label>
          <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-5 gap-2">
            {[
              { key: 'encrypted', label: 'ENCRYPTED (0x01)', desc: 'ChaCha20-Poly1305' },
              { key: 'priority', label: 'PRIORITY (0x02)', desc: 'Urgent queue bypass' },
              { key: 'requiresConsensus', label: 'CONSENSUS (0x04)', desc: 'PoA quorum (T <= N)' },
              { key: 'signed', label: 'SIGNED (0x08)', desc: 'Ed25519 ZSIG trailer' },
              { key: 'broadcast', label: 'BROADCAST (0x10)', desc: 'Address all peers' },
            ].map((f) => {
              const isChecked = flags[f.key as keyof typeof flags];
              return (
                <button
                  key={f.key}
                  onClick={() =>
                    setFlags((prev) => ({
                      ...prev,
                      [f.key]: !prev[f.key as keyof typeof flags],
                    }))
                  }
                  className={`p-3 rounded-xl text-left border transition-all ${
                    isChecked
                      ? 'bg-accent-primary/10 border-accent-primary text-text-primary shadow-glow'
                      : 'bg-bg-subtle border-border-subtle text-text-secondary hover:border-border-strong'
                  }`}
                >
                  <div className="text-xs font-mono font-bold text-accent-primary">
                    {f.label}
                  </div>
                  <div className="text-[10px] text-text-muted mt-1">{f.desc}</div>
                </button>
              );
            })}
          </div>
        </div>

        {/* Node Identifiers & Payload Input */}
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 mb-6">
          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Source Node UUID (16 Bytes)
            </label>
            <input
              type="text"
              value={sourceNode}
              onChange={(e) => setSourceNode(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary"
            />
          </div>
          <div>
            <label className="text-xs font-semibold text-text-primary block mb-1.5">
              Target Node UUID (16 Bytes, all 0s = Broadcast)
            </label>
            <input
              type="text"
              value={flags.broadcast ? '00000000-0000-0000-0000-000000000000' : targetNode}
              onChange={(e) => setTargetNode(e.target.value)}
              disabled={flags.broadcast}
              className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-cyan-300 focus:outline-none focus:border-accent-primary disabled:opacity-50"
            />
          </div>
        </div>

        {/* Payload Editor */}
        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <label className="text-xs font-semibold text-text-primary">
              ZENV Envelope Payload (JSON / Binary)
            </label>
            <span className="text-[11px] font-mono text-text-muted">
              {frameAnalysis.payloadLen} bytes
            </span>
          </div>
          <textarea
            rows={3}
            value={payloadText}
            onChange={(e) => setPayloadText(e.target.value)}
            className="w-full px-3 py-2 rounded-lg bg-bg-subtle border border-border-subtle text-xs font-mono text-text-primary focus:outline-none focus:border-accent-primary"
          />
        </div>
      </div>

      {/* Frame Byte Offset & Architecture Breakdown */}
      <div className="p-6 rounded-2xl border border-border-subtle bg-bg-surface shadow-card">
        <h4 className="text-sm font-bold text-text-primary mb-4 flex items-center gap-2">
          <Layers className="w-4 h-4 text-accent-primary" />
          <span>Real-Time Byte Offset Map (Total Size: {frameAnalysis.totalLen} Bytes)</span>
        </h4>

        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs">
            <thead>
              <tr className="border-b border-border-subtle text-text-muted font-mono">
                <th className="py-2 px-3">Offset</th>
                <th className="py-2 px-3">Field</th>
                <th className="py-2 px-3">Length</th>
                <th className="py-2 px-3">Hex Preview</th>
                <th className="py-2 px-3">Decoded Interpretation</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle/50 font-mono text-text-secondary">
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x00 - 0x03</td>
                <td className="py-2 px-3 font-semibold text-text-primary">magic</td>
                <td className="py-2 px-3">4 B</td>
                <td className="py-2 px-3 text-emerald-400">5A 41 50 5F</td>
                <td className="py-2 px-3 text-text-muted">ASCII &quot;ZAP_&quot; (0x5A41505F)</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x04 - 0x05</td>
                <td className="py-2 px-3 font-semibold text-text-primary">version</td>
                <td className="py-2 px-3">2 B</td>
                <td className="py-2 px-3 text-emerald-400">00 01</td>
                <td className="py-2 px-3 text-text-muted">Version 1</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x06 - 0x07</td>
                <td className="py-2 px-3 font-semibold text-text-primary">flags</td>
                <td className="py-2 px-3">2 B</td>
                <td className="py-2 px-3 text-emerald-400">{frameAnalysis.flagsHex}</td>
                <td className="py-2 px-3 text-cyan-300">Mask: 0x{flagValue.toString(16).padStart(4, '0')}</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x08 - 0x17</td>
                <td className="py-2 px-3 font-semibold text-text-primary">source_node</td>
                <td className="py-2 px-3">16 B</td>
                <td className="py-2 px-3 text-emerald-400">D8 F1 E0 9A ...</td>
                <td className="py-2 px-3 text-text-muted">{sourceNode}</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x18 - 0x27</td>
                <td className="py-2 px-3 font-semibold text-text-primary">target_node</td>
                <td className="py-2 px-3">16 B</td>
                <td className="py-2 px-3 text-emerald-400">
                  {flags.broadcast ? '00 00 00 00 ...' : 'A1 3C 90 7B ...'}
                </td>
                <td className="py-2 px-3 text-text-muted">
                  {flags.broadcast ? 'Broadcast (00..00)' : targetNode}
                </td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x28 - 0x2F</td>
                <td className="py-2 px-3 font-semibold text-text-primary">timestamp_micros</td>
                <td className="py-2 px-3">8 B</td>
                <td className="py-2 px-3 text-emerald-400">00 06 1F 3A ...</td>
                <td className="py-2 px-3 text-text-muted">Unix Microseconds</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x30 - 0x37</td>
                <td className="py-2 px-3 font-semibold text-text-primary">rivun_len</td>
                <td className="py-2 px-3">8 B</td>
                <td className="py-2 px-3 text-emerald-400">{frameAnalysis.lenHex}</td>
                <td className="py-2 px-3 text-text-muted">{frameAnalysis.payloadLen} Bytes</td>
              </tr>
              <tr className="hover:bg-bg-subtle/40">
                <td className="py-2 px-3 text-cyan-400">0x38 - 0x3F</td>
                <td className="py-2 px-3 font-semibold text-text-primary">rivun_sign</td>
                <td className="py-2 px-3">8 B</td>
                <td className="py-2 px-3 text-emerald-400">{frameAnalysis.signHintHex}</td>
                <td className="py-2 px-3 text-text-muted">BLAKE3 Fast Hint</td>
              </tr>
              {flags.signed && (
                <tr className="bg-sky-950/20">
                  <td className="py-2 px-3 text-sky-400">Trailer: ZSIG</td>
                  <td className="py-2 px-3 font-semibold text-sky-300">AuthTrailer</td>
                  <td className="py-2 px-3">72 B</td>
                  <td className="py-2 px-3 text-sky-400">5A 53 49 47 ...</td>
                  <td className="py-2 px-3 text-sky-300">Ed25519 Detached Signature</td>
                </tr>
              )}
              {flags.requiresConsensus && (
                <tr className="bg-purple-950/20">
                  <td className="py-2 px-3 text-purple-400">Trailer: ZPOA</td>
                  <td className="py-2 px-3 font-semibold text-purple-300">PoaTrailer</td>
                  <td className="py-2 px-3">120 B</td>
                  <td className="py-2 px-3 text-purple-400">5A 50 4F 41 ...</td>
                  <td className="py-2 px-3 text-purple-300">PoA Quorum Attestations (T=1..N)</td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
