import React from 'react';
import Link from 'next/link';
import { ChevronRight, Home } from 'lucide-react';
import { BreadcrumbItem } from '@/lib/types';

interface BreadcrumbsProps {
  items: BreadcrumbItem[];
}

export function Breadcrumbs({ items }: BreadcrumbsProps) {
  return (
    <nav aria-label="Breadcrumbs" className="flex items-center space-x-1.5 text-xs text-text-muted mb-4 overflow-x-auto py-1">
      <Link
        href="/"
        className="flex items-center gap-1 hover:text-text-primary transition-colors shrink-0"
      >
        <Home className="w-3.5 h-3.5 text-accent-primary" />
        <span>Docs</span>
      </Link>

      {items.map((item, index) => {
        const isLast = index === items.length - 1;
        return (
          <React.Fragment key={item.href}>
            <ChevronRight className="w-3.5 h-3.5 text-border-strong shrink-0" />
            {isLast ? (
              <span className="font-semibold text-text-primary truncate">
                {item.title}
              </span>
            ) : (
              <Link
                href={item.href}
                className="hover:text-text-primary transition-colors truncate"
              >
                {item.title}
              </Link>
            )}
          </React.Fragment>
        );
      })}
    </nav>
  );
}
