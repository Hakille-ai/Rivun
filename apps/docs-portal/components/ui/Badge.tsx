import React from 'react';

export type BadgeVariant =
  | 'default'
  | 'cyan'
  | 'emerald'
  | 'amber'
  | 'rose'
  | 'purple'
  | 'outline';

interface BadgeProps {
  children: React.ReactNode;
  variant?: BadgeVariant;
  className?: string;
}

const VARIANT_STYLES: Record<BadgeVariant, string> = {
  default: 'bg-bg-subtle text-text-secondary border-border-subtle',
  cyan: 'bg-cyan-500/10 text-cyan-400 border-cyan-500/30',
  emerald: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/30',
  amber: 'bg-amber-500/10 text-amber-400 border-amber-500/30',
  rose: 'bg-rose-500/10 text-rose-400 border-rose-500/30',
  purple: 'bg-purple-500/10 text-purple-400 border-purple-500/30',
  outline: 'bg-transparent text-text-secondary border-border-strong',
};

export function Badge({ children, variant = 'default', className = '' }: BadgeProps) {
  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium border font-mono ${VARIANT_STYLES[variant]} ${className}`}
    >
      {children}
    </span>
  );
}
