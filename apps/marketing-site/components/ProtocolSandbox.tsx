"use client";

import React, { useState, useMemo } from "react";
import {
  Terminal,
  Key,
  ShieldCheck,
  Zap,
  Code,
  Copy,
  Check,
  RotateCcw,
  Sparkles,
  Play,
  Layers,
  ArrowRight,
} from "lucide-react";
import { MessageKindName, ProtocolFlags } from "../lib/types";
import { encodeRivunFrame } from "../lib/protocol";
import {
  deriveNodeIdFromPublicKey,
  fastHash32,
  bytesToHex,
} from "../lib/crypto";

export function ProtocolSandbox() {
  const [kind, setKind] = useState<MessageKindName>("action");
  const [subject, setSubject] = useState("repo.patch");
  const [contentType, setContentType] = useState("application/rivun-agent+json");
  const [payloadJson, setPayloadJson] = useState(
    JSON.stringify({ file: "src/auth/jwt.rs", diff: "+pub fn verify() -> bool { true }" }, null, 2)
  );
  const [flags, setFlags] = useState<ProtocolFlags>({
    signed: true,
    requiresConsensus: true,
    encrypted: false,
    priority: false,
    broadcast: false,
  });

  const [activeLang, setActiveLang] = useState<"rust" | "typescript" | "python" | "go" | "cli">("rust");
  const [copiedCode, setCopiedCode] = useState(false);
  const [keySeed, setKeySeed] = useState("sandbox-agent-key-01");

  // Ephemeral key & Node ID
  const ephemeralPubKey = useMemo(() => {
    return fastHash32("Rivun-PUBKEY-SEED", keySeed);
  }, [keySeed]);

  const nodeId = useMemo(() => {
    return deriveNodeIdFromPublicKey(ephemeralPubKey);
  }, [ephemeralPubKey]);

  const encoded = useMemo(() => {
    try {
      return encodeRivunFrame({
        kind,
        subject,
        contentType,
        payloadJson,
        flags,
        sourceUuidStr: nodeId,
      });
    } catch {
      return encodeRivunFrame({
        kind: "action",
        subject: "error.fallback",
        contentType: "application/json",
        payloadJson: "{}",
        flags,
        sourceUuidStr: nodeId,
      });
    }
  }, [kind, subject, contentType, payloadJson, flags, nodeId]);

  // Policy Evaluation Simulator
  const policyResult = useMemo(() => {
    if (flags.requiresConsensus) {
      return {
        decision: "REQUIRES_POA",
        color: "text-amber-400 bg-amber-500/15 border-amber-500/30",
        message: "Action matches critical security rule: requires 2-of-3 validator signatures.",
      };
    }
    if (subject.includes("delete") || subject.includes("wipe")) {
      return {
        decision: "DENIED",
        color: "text-rose-400 bg-rose-500/15 border-rose-500/30",
        message: "Policy AST violated: destructive action without break-glass token.",
      };
    }
    return {
      decision: "ALLOWED",
      color: "text-[#3DD68C] bg-[#3DD68C]/15 border-[#3DD68C]/30",
      message: "Policy passed: valid Ed25519 signature & verified capability bounds.",
    };
  }, [flags.requiresConsensus, subject]);

  // Code Snippet Generator
  const codeSnippets = useMemo(() => {
    const flagsList = [];
    if (flags.signed) flagsList.push("SIGNED");
    if (flags.requiresConsensus) flagsList.push("REQUIRES_CONSENSUS");
    if (flags.encrypted) flagsList.push("ENCRYPTED");
    if (flags.priority) flagsList.push("PRIORITY");
    if (flags.broadcast) flagsList.push("BROADCAST");
    const flagsStrRust = flagsList.map((f) => `RivunFlags::${f}`).join(" | ") || "RivunFlags::empty()";

    return {
      rust: `// Cargo.toml: rivun-core = "1.0", rivun-envelope = "1.0", rivun-crypto = "1.0"
use rivun_core::{RivunFlags, now_micros};
use rivun_envelope::{RivunEnvelope, RivunMessageKind};
use rivun_crypto::{Keypair, sign_frame};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let keypair = Keypair::from_seed_phrase("${keySeed}")?;
    let node_id = keypair.node_id();

    // 1. Build zero-copy ZENV envelope
    let envelope = RivunEnvelope::builder()
        .kind(RivunMessageKind::${kind.charAt(0).toUpperCase() + kind.slice(1)})
        .subject("${subject}")
        .content_type("${contentType}")
        .body(r#"${payloadJson.replace(/"/g, '\\"')}"#.as_bytes())
        .build()?;

    // 2. Sign wire frame with 64-byte Ed25519 trailer
    let frame = sign_frame(
        &keypair,
        ${flagsStrRust},
        &envelope.encode()?,
    )?;

    println!("Encoded frame size: {} bytes, digest: {}", frame.len(), frame.digest());
    Ok(())
}`,
      typescript: `// npm install @rivun-protocol/sdk
import { RivunClient, RivunFlags, MessageKind } from "@rivun-protocol/sdk";

const client = new RivunClient({
  privateKeySeed: "${keySeed}",
  endpoint: "udp://127.0.0.1:9100",
});

// Construct and sign frame
const frame = await client.createFrame({
  kind: MessageKind.${kind.toUpperCase()},
  subject: "${subject}",
  contentType: "${contentType}",
  payload: ${payloadJson},
  flags: [${flagsList.map((f) => `RivunFlags.${f}`).join(", ")}],
});

console.log(\`Frame \${frame.uuid} ready (\${frame.byteLength} bytes)\`);
await client.send(frame);`,
      python: `# pip install rivun-sdk
from rivun_sdk import RivunNode, MessageKind, RivunFlags
import json

node = RivunNode.from_seed("${keySeed}")

frame = node.create_signed_frame(
    kind=MessageKind.${kind.toUpperCase()},
    subject="${subject}",
    content_type="${contentType}",
    body=json.dumps(${payloadJson}).encode("utf-8"),
    flags=[${flagsList.map((f) => `RivunFlags.${f}`).join(", ")}],
)

print(f"Node UUID: {node.node_id} | Encoded Frame: {len(frame.raw_bytes)} bytes")
node.broadcast(frame)`,
      go: `// go get github.com/Hakille-ai/ZAP/sdks/go/rivun
package main

import (
	"fmt"
	"github.com/Hakille-ai/ZAP/sdks/go/rivun"
)

func main() {
	keypair, _ := rivun.KeypairFromSeed("${keySeed}")
	client := rivun.NewClient(keypair)

	envelope := rivun.NewEnvelope(
		rivun.Kind${kind.charAt(0).toUpperCase() + kind.slice(1)},
		"${subject}",
		"${contentType}",
		[]byte(\`${payloadJson}\`),
	)

	signedFrame, err := client.SignFrame(envelope, rivun.Flag${flagsList[0] || "Signed"})
	if err != nil {
		panic(err)
	}

	fmt.Printf("Signed Frame digest: %s (%d bytes)\\n", signedFrame.Digest(), signedFrame.Len())
}`,
      cli: `# CLI Dispatch Command (rivun-cli)
rivun send \\
  --config ~/.rivun/config.toml \\
  --kind ${kind} \\
  --subject "${subject}" \\
  --content-type "${contentType}" \\
  --payload '${payloadJson.replace(/\n/g, "")}' \\
  ${flags.requiresConsensus ? "--requires-consensus " : ""}${flags.priority ? "--priority " : ""}${flags.broadcast ? "--broadcast" : ""}`,
    };
  }, [kind, subject, contentType, payloadJson, flags, keySeed]);

  const copyCode = () => {
    navigator.clipboard.writeText(codeSnippets[activeLang]);
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 2000);
  };

  return (
    <section className="py-12 relative">
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        {/* Header */}
        <div className="text-center max-w-3xl mx-auto mb-12 space-y-4">
          <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-[#5B8CFF]/10 border border-[#5B8CFF]/20 text-[#5B8CFF] text-xs font-semibold">
            <Terminal className="w-3.5 h-3.5" />
            <span>DEVELOPER SANDBOX & PLAYGROUND</span>
          </div>
          <h2 className="text-3xl sm:text-4xl font-extrabold tracking-tight text-white">
            Author, Sign & Verify Wire Frames in Seconds
          </h2>
          <p className="text-sm sm:text-base text-[#9AA1AE]">
            Generate ephemeral Ed25519 keypairs, test AST policies, and generate production-ready code in 5 programming languages.
          </p>
        </div>

        {/* Main Sandbox Card */}
        <div className="bg-[#111318] border border-[#22262F] rounded-2xl p-6 lg:p-8 shadow-2xl space-y-6">
          {/* Top Bar: Ephemeral Key & Node ID */}
          <div className="p-4 rounded-xl bg-[#0A0B0D] border border-[#22262F] flex flex-col md:flex-row md:items-center justify-between gap-4">
            <div className="flex items-center gap-3">
              <div className="p-2.5 rounded-lg bg-[#5B8CFF]/15 text-[#5B8CFF] border border-[#5B8CFF]/30">
                <Key className="w-5 h-5" />
              </div>
              <div>
                <span className="text-[10px] font-mono text-[#6B7280] uppercase block">
                  Active Node Identity (UUIDv8)
                </span>
                <span className="text-xs font-mono font-bold text-white">{nodeId}</span>
              </div>
            </div>

            <div className="flex items-center gap-2">
              <button
                onClick={() => setKeySeed(`agent-seed-${Math.floor(Math.random() * 9000 + 1000)}`)}
                className="px-3 py-1.5 rounded-lg bg-[#181B22] hover:bg-[#22262F] border border-[#22262F] text-xs text-[#9AA1AE] hover:text-white transition-all flex items-center gap-1.5"
              >
                <RotateCcw className="w-3.5 h-3.5" />
                <span>Rotate Key</span>
              </button>
            </div>
          </div>

          {/* Form & Policy Evaluator Layout */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
            {/* Left: Input Controls (5 cols) */}
            <div className="lg:col-span-5 space-y-4">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                    Kind
                  </label>
                  <select
                    value={kind}
                    onChange={(e) => setKind(e.target.value as MessageKindName)}
                    className="w-full px-3 py-2 text-xs bg-[#181B22] border border-[#22262F] rounded-lg text-white outline-none"
                  >
                    <option value="action">Action (0x07)</option>
                    <option value="data">Data (0x01)</option>
                    <option value="event">Event (0x02)</option>
                    <option value="command">Command (0x03)</option>
                    <option value="query">Query (0x04)</option>
                    <option value="control">Control (0x08)</option>
                  </select>
                </div>
                <div>
                  <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                    Subject
                  </label>
                  <input
                    type="text"
                    value={subject}
                    onChange={(e) => setSubject(e.target.value)}
                    className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] rounded-lg text-white outline-none"
                  />
                </div>
              </div>

              <div>
                <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                  Content-Type
                </label>
                <input
                  type="text"
                  value={contentType}
                  onChange={(e) => setContentType(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] rounded-lg text-white outline-none"
                />
              </div>

              <div>
                <label className="text-xs font-semibold text-[#9AA1AE] block mb-1">
                  Payload JSON
                </label>
                <textarea
                  rows={4}
                  value={payloadJson}
                  onChange={(e) => setPayloadJson(e.target.value)}
                  className="w-full px-3 py-2 text-xs font-mono bg-[#181B22] border border-[#22262F] rounded-lg text-emerald-400 outline-none resize-none"
                />
              </div>

              {/* Flags */}
              <div className="flex flex-wrap gap-2 pt-1">
                <button
                  onClick={() => setFlags((f) => ({ ...f, signed: !f.signed }))}
                  className={`px-2.5 py-1 text-[11px] font-semibold rounded-lg border transition-all ${
                    flags.signed
                      ? "bg-[#3DD68C]/15 text-[#3DD68C] border-[#3DD68C]/40"
                      : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                  }`}
                >
                  SIGNED
                </button>
                <button
                  onClick={() => setFlags((f) => ({ ...f, requiresConsensus: !f.requiresConsensus }))}
                  className={`px-2.5 py-1 text-[11px] font-semibold rounded-lg border transition-all ${
                    flags.requiresConsensus
                      ? "bg-amber-400/15 text-amber-400 border-amber-400/40"
                      : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                  }`}
                >
                  POA CONSENSUS
                </button>
                <button
                  onClick={() => setFlags((f) => ({ ...f, priority: !f.priority }))}
                  className={`px-2.5 py-1 text-[11px] font-semibold rounded-lg border transition-all ${
                    flags.priority
                      ? "bg-rose-400/15 text-rose-400 border-rose-400/40"
                      : "bg-[#181B22] text-[#6B7280] border-[#22262F]"
                  }`}
                >
                  PRIORITY
                </button>
              </div>

              {/* Policy Rule Decision Card */}
              <div className="p-3.5 rounded-xl bg-[#14171F] border border-[#22262F] space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-[10px] font-mono text-[#6B7280] uppercase">
                    Simulated Policy Evaluator
                  </span>
                  <span className={`px-2 py-0.5 rounded text-[10px] font-mono font-bold border ${policyResult.color}`}>
                    {policyResult.decision}
                  </span>
                </div>
                <p className="text-xs text-[#9AA1AE] leading-relaxed">
                  {policyResult.message}
                </p>
              </div>
            </div>

            {/* Right: Multi-Language Code Viewer (7 cols) */}
            <div className="lg:col-span-7 bg-[#0A0B0D] border border-[#22262F] rounded-xl overflow-hidden shadow-inner flex flex-col">
              {/* Language Switcher Tabs */}
              <div className="flex items-center justify-between px-4 py-2.5 bg-[#14171F] border-b border-[#22262F]">
                <div className="flex items-center gap-1.5 overflow-x-auto">
                  {(["rust", "typescript", "python", "go", "cli"] as const).map((lang) => (
                    <button
                      key={lang}
                      onClick={() => setActiveLang(lang)}
                      className={`px-3 py-1 text-xs font-mono font-medium rounded-lg transition-all capitalize ${
                        activeLang === lang
                          ? "bg-[#5B8CFF] text-white shadow-sm"
                          : "text-[#9AA1AE] hover:text-white"
                      }`}
                    >
                      {lang}
                    </button>
                  ))}
                </div>

                <button
                  onClick={copyCode}
                  className="px-2.5 py-1 rounded bg-[#181B22] hover:bg-[#22262F] text-[#9AA1AE] hover:text-white border border-[#22262F] transition-all text-xs flex items-center gap-1"
                >
                  {copiedCode ? <Check className="w-3.5 h-3.5 text-[#3DD68C]" /> : <Copy className="w-3.5 h-3.5" />}
                  <span>{copiedCode ? "Copied" : "Copy"}</span>
                </button>
              </div>

              {/* Code Editor Body */}
              <div className="p-4 font-mono text-xs overflow-x-auto text-[#9AA1AE] flex-1 leading-relaxed">
                <pre className="text-gray-200">
                  <code>{codeSnippets[activeLang]}</code>
                </pre>
              </div>

              {/* Status Bar */}
              <div className="px-4 py-2 bg-[#14171F] border-t border-[#22262F] flex items-center justify-between text-[11px] font-mono text-[#6B7280]">
                <span>Total Frame: {encoded.totalSize} Bytes</span>
                <span className="text-[#3DD68C]">Wire Magic: 0x5A41505F (ZAP_)</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}
