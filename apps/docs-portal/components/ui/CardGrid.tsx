import React from 'react';
import Link from 'next/link';
import { ArrowRight } from 'lucide-react';
import { Badge } from '@/components/ui/Badge';

export interface CardItem {
  title: string;
  description: string;
  href: string;
  icon?: React.ReactNode;
  badge?: string;
  tag?: string;
}

interface CardGridProps {
  items: CardItem[];
  columns?: 2 | 3 | 4;
}

export function CardGrid({ items, columns = 3 }: CardGridProps) {
  const colClass =
    columns === 2
      ? 'grid-cols-1 md:grid-cols-2'
      : columns === 4
      ? 'grid-cols-1 sm:grid-cols-2 lg:grid-cols-4'
      : 'grid-cols-1 md:grid-cols-2 lg:grid-cols-3';

  return (
    <div className={`grid gap-4 my-6 ${colClass}`}>
      {items.map((item, idx) => (
        <Link
          key={idx}
          href={item.href}
          className="group relative flex flex-col justify-between p-5 rounded-xl border border-border-subtle bg-bg-surface/80 hover:bg-bg-surface-raised transition-all duration-200 hover:border-accent-primary/50 shadow-card hover:shadow-glow"
        >
          <div>
            <div className="flex items-center justify-between mb-3">
              {item.icon ? (
                <div className="p-2 rounded-lg bg-bg-subtle border border-border-subtle text-accent-primary group-hover:scale-110 transition-transform">
                  {item.icon}
                </div>
              ) : (
                <div />
              )}
              {item.badge && <Badge variant="cyan">{item.badge}</Badge>}
            </div>
            <h3 className="text-base font-semibold text-text-primary group-hover:text-accent-primary transition-colors flex items-center gap-1.5">
              {item.title}
            </h3>
            <p className="mt-1.5 text-xs leading-relaxed text-text-secondary">
              {item.description}
            </p>
          </div>

          <div className="mt-4 flex items-center gap-1 text-xs font-medium text-accent-primary opacity-80 group-hover:opacity-100 group-hover:translate-x-0.5 transition-all">
            <span>Explore documentation</span>
            <ArrowRight className="w-3.5 h-3.5" />
          </div>
        </Link>
      ))}
    </div>
  );
}
