'use client';

import React, { useState, useEffect } from 'react';
import { Check, Copy } from 'lucide-react';

export interface CodeTabItem {
  language: 'rust' | 'typescript' | 'python' | 'go' | 'bash' | 'cli' | 'toml' | 'json';
  title?: string;
  code: string;
}

interface CodeTabsProps {
  tabs: Record<string, { title: string; code: string }>;
  defaultTab?: string;
}

const LANGUAGE_LABELS: Record<string, string> = {
  rust: 'Rust',
  typescript: 'TypeScript',
  python: 'Python',
  go: 'Go',
  bash: 'CLI / Bash',
  cli: 'CLI',
  toml: 'TOML',
  json: 'JSON',
};

export function CodeTabs({ tabs, defaultTab }: CodeTabsProps) {
  const keys = Object.keys(tabs);
  const [activeTab, setActiveTab] = useState<string>(() => {
    if (defaultTab && keys.includes(defaultTab)) return defaultTab;
    return keys[0] || 'rust';
  });
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    try {
      const saved = localStorage.getItem('rivun_docs_lang_pref');
      if (saved && keys.includes(saved)) {
        setActiveTab(saved);
      }
    } catch {
      // ignore
    }
  }, [keys]);

  const handleSelectTab = (key: string) => {
    setActiveTab(key);
    try {
      localStorage.setItem('rivun_docs_lang_pref', key);
    } catch {
      // ignore
    }
  };

  const currentTab = tabs[activeTab] || tabs[keys[0]];
  if (!currentTab) return null;

  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(currentTab.code);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy:', err);
    }
  };

  return (
    <div className="my-6 rounded-xl border border-border-subtle bg-[#0B0F17] overflow-hidden shadow-card">
      {/* Tabs navigation bar */}
      <div className="flex items-center justify-between px-2 bg-[#0F1420] border-b border-border-subtle overflow-x-auto">
        <div className="flex items-center gap-1">
          {keys.map((key) => {
            const isActive = key === activeTab;
            return (
              <button
                key={key}
                onClick={() => handleSelectTab(key)}
                className={`px-3 py-2.5 text-xs font-medium transition-all relative border-b-2 ${
                  isActive
                    ? 'text-accent-primary border-accent-primary bg-bg-surface/50'
                    : 'text-text-secondary border-transparent hover:text-text-primary hover:bg-bg-surface-raised/50'
                }`}
              >
                {tabs[key].title || LANGUAGE_LABELS[key] || key.toUpperCase()}
              </button>
            );
          })}
        </div>

        <button
          onClick={handleCopy}
          aria-label="Copy tab code"
          className="flex items-center gap-1.5 px-2.5 py-1 my-1.5 rounded-md bg-bg-surface-raised hover:bg-border-subtle text-text-secondary hover:text-text-primary transition-all text-xs border border-border-subtle select-none"
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

      {/* Code panel */}
      <div className="p-4 overflow-x-auto text-sm font-mono leading-relaxed text-[#E2E8F0] selection:bg-accent-primary/30">
        <pre className="m-0 p-0">
          <code>{currentTab.code.trim()}</code>
        </pre>
      </div>
    </div>
  );
}
