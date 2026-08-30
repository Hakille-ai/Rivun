import React from 'react';
import { Layers } from 'lucide-react';

interface MermaidProps {
  chart: string;
  title?: string;
}

export function Mermaid({ chart, title }: MermaidProps) {
  return (
    <div className="my-6 rounded-xl border border-border-subtle bg-bg-surface overflow-hidden shadow-card">
      {/* Header bar */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-bg-surface-raised border-b border-border-subtle text-xs text-text-secondary">
        <div className="flex items-center gap-2">
          <Layers className="w-4 h-4 text-accent-primary" />
          <span className="font-semibold text-text-primary">
            {title || 'Architecture & Sequence Flow'}
          </span>
        </div>
        <span className="text-[10px] font-mono text-cyan-400 bg-cyan-500/10 px-2 py-0.5 rounded border border-cyan-500/20">
          ZAP Protocol Visualizer
        </span>
      </div>

      {/* Diagram container */}
      <div className="p-6 overflow-x-auto flex justify-center items-center bg-[#07090E]/60 min-h-[160px]">
        <div className="font-mono text-xs text-cyan-300 p-4 rounded-lg bg-bg-surface-raised/80 border border-border-subtle w-full max-w-2xl shadow-card">
          <pre className="whitespace-pre-wrap text-text-secondary leading-relaxed overflow-x-auto selection:bg-accent-primary/30">
            {chart.trim()}
          </pre>
        </div>
      </div>
    </div>
  );
}
