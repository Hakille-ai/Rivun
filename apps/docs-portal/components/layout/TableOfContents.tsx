'use client';

import React, { useEffect, useState } from 'react';
import { AlignLeft } from 'lucide-react';
import { HeadingItem } from '@/lib/types';

interface TableOfContentsProps {
  headings: HeadingItem[];
}

export function TableOfContents({ headings }: TableOfContentsProps) {
  const [activeId, setActiveId] = useState<string>('');

  useEffect(() => {
    if (headings.length === 0) return;

    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            setActiveId(entry.target.id);
          }
        });
      },
      { rootMargin: '0px 0px -60% 0px' }
    );

    headings.forEach((h) => {
      const el = document.getElementById(h.id);
      if (el) observer.observe(el);
    });

    return () => observer.disconnect();
  }, [headings]);

  if (headings.length === 0) return null;

  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2 text-xs font-bold text-text-primary uppercase tracking-wider">
        <AlignLeft className="w-3.5 h-3.5 text-accent-primary" />
        <span>On This Page</span>
      </div>

      <nav className="space-y-1 text-xs">
        {headings.map((heading) => {
          const isActive = activeId === heading.id;
          return (
            <a
              key={heading.id}
              href={`#${heading.id}`}
              className={`block py-1 transition-all leading-snug ${
                heading.level === 3 ? 'pl-3' : 'pl-0'
              } ${
                isActive
                  ? 'text-accent-primary font-semibold border-l-2 border-accent-primary pl-2'
                  : 'text-text-secondary hover:text-text-primary'
              }`}
            >
              {heading.text}
            </a>
          );
        })}
      </nav>
    </div>
  );
}
