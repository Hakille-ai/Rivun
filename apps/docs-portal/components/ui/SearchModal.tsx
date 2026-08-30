'use client';

import React, { useState, useEffect, useRef } from 'react';
import { useRouter } from 'next/navigation';
import {
  Search,
  X,
  ArrowRight,
  Sparkles,
  Command,
  CornerDownLeft,
} from 'lucide-react';
import { globalSearchEngine, SearchResult } from '@/lib/search-index';
import { generateSearchIndex } from '@/lib/docs-content';

interface SearchModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const CATEGORIES = [
  'All',
  'Protocol',
  'Crates',
  'SDKs',
  'Domain Packs',
  'Cloud',
  'Operations',
  'Tools',
];

export function SearchModal({ isOpen, onClose }: SearchModalProps) {
  const [query, setQuery] = useState('');
  const [category, setCategory] = useState('All');
  const [results, setResults] = useState<SearchResult[]>([]);
  const [selectedIndex, setSelectedIndex] = useState(0);
  const router = useRouter();
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    // Populate engine with default docs
    const records = generateSearchIndex();
    globalSearchEngine.setRecords(records);
    globalSearchEngine.loadFromPublic().catch(() => {});
  }, []);

  useEffect(() => {
    if (isOpen) {
      setTimeout(() => inputRef.current?.focus(), 50);
      setSelectedIndex(0);
    } else {
      setQuery('');
      setResults([]);
    }
  }, [isOpen]);

  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      return;
    }
    const res = globalSearchEngine.search(query, category);
    setResults(res);
    setSelectedIndex(0);
  }, [query, category]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (!isOpen) return;

      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        setSelectedIndex((prev) => (results.length > 0 ? (prev + 1) % results.length : 0));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setSelectedIndex((prev) =>
          results.length > 0 ? (prev - 1 + results.length) % results.length : 0
        );
      } else if (e.key === 'Enter') {
        if (results.length > 0 && results[selectedIndex]) {
          e.preventDefault();
          router.push(results[selectedIndex].record.url);
          onClose();
        }
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, results, selectedIndex, router, onClose]);

  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-20 px-4">
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/70 backdrop-blur-md transition-opacity"
        onClick={onClose}
      />

      {/* Modal Container */}
      <div className="relative w-full max-w-2xl rounded-2xl glass-modal border border-border-strong overflow-hidden shadow-modal animate-in fade-in zoom-in-95 duration-150">
        {/* Search Input Bar */}
        <div className="flex items-center px-4 py-3.5 border-b border-border-subtle bg-bg-surface-raised/90">
          <Search className="w-5 h-5 text-accent-primary shrink-0 mr-3" />
          <input
            ref={inputRef}
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search all 26 crates, SDKs, protocols, wire formats, consensus..."
            className="w-full bg-transparent text-sm text-text-primary placeholder:text-text-muted focus:outline-none font-sans"
          />
          {query ? (
            <button
              onClick={() => setQuery('')}
              className="p-1 rounded hover:bg-bg-subtle text-text-secondary hover:text-text-primary mr-2"
            >
              <X className="w-4 h-4" />
            </button>
          ) : null}
          <kbd className="hidden sm:flex items-center gap-0.5 px-2 py-0.5 rounded border border-border-subtle bg-bg-surface text-[10px] font-mono text-text-muted">
            ESC
          </kbd>
        </div>

        {/* Category Filters */}
        <div className="flex items-center gap-1.5 px-4 py-2 bg-bg-surface border-b border-border-subtle overflow-x-auto text-xs">
          {CATEGORIES.map((cat) => (
            <button
              key={cat}
              onClick={() => setCategory(cat)}
              className={`px-2.5 py-1 rounded-lg text-xs font-medium transition-all whitespace-nowrap ${
                category === cat
                  ? 'bg-accent-primary text-bg-base font-semibold'
                  : 'bg-bg-subtle/80 text-text-secondary hover:text-text-primary hover:bg-bg-subtle'
              }`}
            >
              {cat}
            </button>
          ))}
        </div>

        {/* Results List */}
        <div className="max-h-96 overflow-y-auto p-2 space-y-1">
          {query.trim() === '' ? (
            <div className="py-12 px-6 text-center text-text-secondary">
              <Sparkles className="w-8 h-8 text-accent-primary mx-auto mb-3 opacity-60" />
              <p className="text-sm font-medium text-text-primary">
                Instant Sub-10ms Full-Text Search
              </p>
              <p className="text-xs text-text-muted mt-1">
                Type crate names, protocol constants (e.g. <code>ZAP_</code>, <code>ZENV</code>, <code>ZSIG</code>), or SDK methods.
              </p>
            </div>
          ) : results.length === 0 ? (
            <div className="py-12 px-6 text-center text-text-muted text-sm">
              No documentation results found for &ldquo;<span className="text-text-primary">{query}</span>&rdquo;
            </div>
          ) : (
            results.map((res, idx) => {
              const isSelected = idx === selectedIndex;
              return (
                <div
                  key={res.record.id}
                  onClick={() => {
                    router.push(res.record.url);
                    onClose();
                  }}
                  onMouseEnter={() => setSelectedIndex(idx)}
                  className={`flex flex-col p-3 rounded-xl cursor-pointer transition-all ${
                    isSelected
                      ? 'bg-accent-primary/10 border border-accent-primary/40'
                      : 'hover:bg-bg-surface-raised/50 border border-transparent'
                  }`}
                >
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="px-2 py-0.5 rounded text-[10px] font-mono font-medium bg-bg-subtle border border-border-subtle text-accent-primary">
                        {res.record.section}
                      </span>
                      <span className="text-sm font-semibold text-text-primary">
                        {res.record.title}
                      </span>
                    </div>
                    {isSelected && (
                      <div className="flex items-center gap-1 text-[11px] text-accent-primary font-mono">
                        <span>Open</span>
                        <CornerDownLeft className="w-3 h-3" />
                      </div>
                    )}
                  </div>
                  <p className="mt-1 text-xs text-text-secondary line-clamp-1">
                    {res.matchedSnippet}
                  </p>
                </div>
              );
            })
          )}
        </div>

        {/* Footer info */}
        <div className="flex items-center justify-between px-4 py-2 bg-bg-surface-raised/60 border-t border-border-subtle text-[11px] text-text-muted">
          <div className="flex items-center gap-3">
            <span className="flex items-center gap-1">
              <kbd className="px-1 py-0.5 rounded bg-bg-surface border border-border-subtle text-[10px]">↑</kbd>
              <kbd className="px-1 py-0.5 rounded bg-bg-surface border border-border-subtle text-[10px]">↓</kbd> Navigate
            </span>
            <span className="flex items-center gap-1">
              <kbd className="px-1 py-0.5 rounded bg-bg-surface border border-border-subtle text-[10px]">↵</kbd> Select
            </span>
          </div>
          <span>&lt; 10ms Search Latency</span>
        </div>
      </div>
    </div>
  );
}
