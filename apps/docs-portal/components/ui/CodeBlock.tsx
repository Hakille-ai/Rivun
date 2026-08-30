'use client';

import React, { useState } from 'react';
import { Check, Copy, Terminal } from 'lucide-react';

interface CodeBlockProps {
  code: string;
  language?: string;
  filename?: string;
  showLineNumbers?: boolean;
}

export function CodeBlock({
  code,
  language = 'bash',
  filename,
  showLineNumbers = false,
}: CodeBlockProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy code:', err);
    }
  };

  const lines = code.trim().split('\n');

  return (
    <div className="my-5 rounded-xl border border-border-subtle bg-[#0B0F17] overflow-hidden shadow-card group">
      {/* Header bar */}
      <div className="flex items-center justify-between px-4 py-2.5 bg-[#0F1420] border-b border-border-subtle text-xs text-text-secondary">
        <div className="flex items-center gap-2 font-mono">
          <Terminal className="w-3.5 h-3.5 text-accent-primary" />
          <span className="font-semibold text-text-primary">
            {filename || language.toUpperCase()}
          </span>
        </div>
        <button
          onClick={handleCopy}
          aria-label="Copy code"
          className="flex items-center gap-1.5 px-2.5 py-1 rounded-md bg-bg-surface-raised hover:bg-border-subtle text-text-secondary hover:text-text-primary transition-all text-xs border border-border-subtle"
        >
          {copied ? (
            <>
              <Check className="w-3.5 h-3.5 text-status-verified" />
              <span className="text-status-verified font-medium">Copied!</span>
            </>
          ) : (
            <>
              <Copy className="w-3.5 h-3.5" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      {/* Code body */}
      <div className="p-4 overflow-x-auto text-sm font-mono leading-relaxed text-[#E2E8F0] selection:bg-accent-primary/30">
        <pre className="m-0 p-0">
          <code>
            {showLineNumbers
              ? lines.map((line, i) => (
                  <div key={i} className="table-row">
                    <span className="table-cell pr-4 text-right select-none text-text-muted text-xs opacity-50">
                      {i + 1}
                    </span>
                    <span className="table-cell">{line}</span>
                  </div>
                ))
              : code.trim()}
          </code>
        </pre>
      </div>
    </div>
  );
}
