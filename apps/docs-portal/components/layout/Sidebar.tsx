'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { usePathname } from 'next/navigation';
import {
  ChevronDown,
  ChevronRight,
  Rocket,
  Layers,
  Cpu,
  Boxes,
  Cloud,
  Package,
  Code,
  Store,
  ShieldCheck,
  PlayCircle,
  Filter,
} from 'lucide-react';
import { DOCS_NAVIGATION } from '@/lib/navigation';
import { NavSection } from '@/lib/types';

const SECTION_ICONS: Record<string, React.ElementType> = {
  Rocket,
  Layers,
  Cpu,
  Boxes,
  Cloud,
  Package,
  Code,
  Store,
  ShieldCheck,
  PlayCircle,
};

interface SidebarProps {
  onCloseMobile?: () => void;
}

export function Sidebar({ onCloseMobile }: SidebarProps) {
  const pathname = usePathname();
  const [filterQuery, setFilterQuery] = useState('');
  const [collapsedSections, setCollapsedSections] = useState<Record<string, boolean>>({});

  const toggleSection = (title: string) => {
    setCollapsedSections((prev) => ({
      ...prev,
      [title]: !prev[title],
    }));
  };

  const filteredSections: NavSection[] = DOCS_NAVIGATION.map((section) => {
    if (!filterQuery.trim()) return section;
    const query = filterQuery.toLowerCase();
    const matchingItems = section.items.filter(
      (item) =>
        item.title.toLowerCase().includes(query) ||
        (item.badge && item.badge.toLowerCase().includes(query))
    );
    return {
      ...section,
      items: matchingItems,
    };
  }).filter((section) => section.items.length > 0);

  return (
    <aside className="w-full h-full flex flex-col bg-bg-surface/95 border-r border-border-subtle overflow-hidden">
      {/* Sidebar search / filter bar */}
      <div className="p-3 border-b border-border-subtle">
        <div className="relative">
          <Filter className="w-3.5 h-3.5 text-text-muted absolute left-2.5 top-2.5" />
          <input
            type="text"
            value={filterQuery}
            onChange={(e) => setFilterQuery(e.target.value)}
            placeholder="Quick filter topics..."
            className="w-full pl-8 pr-3 py-1.5 rounded-lg bg-bg-subtle text-xs text-text-primary placeholder:text-text-muted focus:outline-none border border-border-subtle focus:border-accent-primary/50"
          />
        </div>
      </div>

      {/* Navigation Section List */}
      <div className="flex-1 overflow-y-auto px-3 py-4 space-y-5">
        {filteredSections.map((section) => {
          const isCollapsed = Boolean(collapsedSections[section.title]);
          const Icon = section.icon ? SECTION_ICONS[section.icon] || Layers : Layers;

          return (
            <div key={section.title} className="space-y-1">
              {/* Section Header Accordion Trigger */}
              <button
                onClick={() => toggleSection(section.title)}
                className="w-full flex items-center justify-between px-2 py-1.5 text-xs font-bold text-text-primary uppercase tracking-wider hover:text-accent-primary transition-colors select-none"
              >
                <div className="flex items-center gap-2">
                  <Icon className="w-3.5 h-3.5 text-accent-primary" />
                  <span>{section.title}</span>
                </div>
                {isCollapsed ? (
                  <ChevronRight className="w-3.5 h-3.5 text-text-muted" />
                ) : (
                  <ChevronDown className="w-3.5 h-3.5 text-text-muted" />
                )}
              </button>

              {/* Items List */}
              {!isCollapsed && (
                <div className="space-y-0.5 pt-1 pl-2 border-l border-border-subtle ml-2">
                  {section.items.map((item) => {
                    const isActive = pathname === item.href;
                    return (
                      <Link
                        key={item.href}
                        href={item.href}
                        onClick={onCloseMobile}
                        className={`flex items-center justify-between px-2.5 py-1.5 rounded-lg text-xs transition-all group ${
                          isActive
                            ? 'text-accent-primary bg-accent-primary/10 font-semibold border-l-2 border-accent-primary'
                            : 'text-text-secondary hover:text-text-primary hover:bg-bg-subtle/70'
                        }`}
                      >
                        <span className="truncate">{item.title}</span>
                        {item.badge && (
                          <span
                            className={`ml-2 text-[10px] font-mono px-1.5 py-0.2 rounded border ${
                              isActive
                                ? 'bg-cyan-500/20 text-cyan-300 border-cyan-500/40'
                                : 'bg-bg-subtle text-text-muted border-border-subtle group-hover:text-text-secondary'
                            }`}
                          >
                            {item.badge}
                          </span>
                        )}
                      </Link>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </aside>
  );
}
