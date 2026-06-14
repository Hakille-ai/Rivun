import Link from 'next/link';
import { ArrowLeft } from 'lucide-react';
import Footer from "@/components/layout/Footer";

export const metadata = {
  title: 'Vision | ZAP Protocol',
  description: 'The vision behind the ZAP Protocol: Building the nervous system for the next generation of autonomous distributed systems.',
};

export default function VisionPage() {
  return (
    <div className="flex-1 overflow-y-auto w-full">
      <div className="flex flex-col items-center justify-start min-h-screen pt-24 pb-32 px-6">
      <div className="w-full max-w-3xl relative z-10">
        <Link href="/" className="inline-flex items-center gap-2 text-zinc-400 hover:text-white transition-colors mb-12">
          <ArrowLeft className="w-4 h-4" /> Back to Home
        </Link>
        
        <div className="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-blue-500/10 border border-blue-500/20 text-blue-400 text-sm font-medium mb-8">
          Manifesto
        </div>
        
        <h1 className="text-4xl md:text-6xl font-extrabold tracking-tight mb-8">
          Building the Nervous System for Autonomous Systems
        </h1>
        
        <div className="prose prose-invert prose-lg max-w-none text-zinc-300">
          <p className="lead text-xl text-zinc-400 mb-10">
            We are entering a new era of computing where AI agents, physical robots, and edge devices need to coordinate with nanosecond latency, cryptographic certainty, and zero trust. 
            The protocols of the past were not built for this future.
          </p>

          <h2 className="text-2xl font-bold text-white mt-12 mb-6">The Problem with Legacy Protocols</h2>
          <p className="mb-6">
            Current distributed systems rely on brokers (MQTT), heavy connection-oriented protocols (gRPC/TCP), or centralized ledgers (Kafka). 
            When deploying autonomous systems in the real world—like a swarm of drones, a smart factory floor, or a multi-agent AI pipeline—these legacy architectures introduce unacceptable trade-offs.
          </p>
          <ul className="space-y-4 mb-8 list-disc pl-6 text-zinc-400">
            <li><strong className="text-zinc-200">Central points of failure:</strong> If the broker goes down, the swarm dies.</li>
            <li><strong className="text-zinc-200">Head-of-line blocking:</strong> TCP retransmissions can delay critical safety signals in noisy RF environments.</li>
            <li><strong className="text-zinc-200">Implicit Trust:</strong> Once inside the network, services are often implicitly trusted to perform actions they shouldn't.</li>
          </ul>

          <h2 className="text-2xl font-bold text-white mt-12 mb-6">The ZAP Philosophy</h2>
          <p className="mb-6">
            ZAP was designed from first principles to solve these challenges. We believe that:
          </p>
          <ol className="space-y-6 mb-8 list-decimal pl-6">
            <li>
              <strong className="text-white text-lg block mb-2">1. Every action must have cryptographic provenance.</strong>
              <span className="text-zinc-400">In ZAP, every single 64-byte frame is Ed25519-signed. We don't just secure the connection; we secure the message. If an AI agent commands a robot to move, that command is cryptographically tied to the agent's identity forever.</span>
            </li>
            <li>
              <strong className="text-white text-lg block mb-2">2. Execution must be sandboxed.</strong>
              <span className="text-zinc-400">You shouldn't have to trust the code you run. ZAP embeds a WebAssembly (WASM) runtime to execute message drivers. By default, drivers cannot access the network, filesystem, or clock. They are given a strict fuel limit and memory bound.</span>
            </li>
            <li>
              <strong className="text-white text-lg block mb-2">3. Consensus is a primitive.</strong>
              <span className="text-zinc-400">Critical actions—like an emergency stop on a factory floor—shouldn't rely on a single node's decision. ZAP integrates Proof-of-Action (PoA), requiring a quorum of validators to cryptographically attest to an action before it is executed.</span>
            </li>
            <li>
              <strong className="text-white text-lg block mb-2">4. Latency is safety.</strong>
              <span className="text-zinc-400">By using decentralized, encrypted UDP (ChaCha20-Poly1305), ZAP eliminates connection setup overhead and head-of-line blocking. Our zero-copy parsers process frames in nanoseconds.</span>
            </li>
          </ol>

          <h2 className="text-2xl font-bold text-white mt-12 mb-6">Beyond AI</h2>
          <p className="mb-6">
            While ZAP is the perfect protocol for multi-agent LLM systems, it is completely agnostic to artificial intelligence. 
            It is a deterministic, low-level binary protocol that can run on an industrial programmable logic controller (PLC) just as easily as it runs on a cloud server.
          </p>
          
          <div className="glass-panel p-8 rounded-xl mt-12 border-blue-500/30">
            <h3 className="text-xl font-bold text-white mb-4">Join the Alpha</h3>
            <p className="text-zinc-400 mb-6">
              ZAP is currently in Pre-1.0 Alpha. We are actively looking for design partners building mission-critical distributed systems.
            </p>
            <Link href="/docs/getting-started" className="inline-flex bg-white text-black hover:bg-zinc-200 px-6 py-3 rounded-full font-semibold transition-all">
              Read the Docs
            </Link>
          </div>
        </div>
      </div>
      </div>
      <div className="glow-bg top-1/4 right-0 opacity-50"></div>
      <Footer />
    </div>
  );
}
