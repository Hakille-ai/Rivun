import React from 'react';
import {
  Info,
  Sparkles,
  AlertCircle,
  AlertTriangle,
  ShieldAlert,
  Lock,
} from 'lucide-react';

export type CalloutType =
  | 'note'
  | 'tip'
  | 'important'
  | 'warning'
  | 'danger'
  | 'security'
  | 'invariant';

interface CalloutProps {
  type?: CalloutType;
  title?: string;
  children: React.ReactNode;
}

const CALLOUT_CONFIG: Record<
  CalloutType,
  {
    icon: React.ElementType;
    borderColor: string;
    bgColor: string;
    iconColor: string;
    defaultTitle: string;
  }
> = {
  note: {
    icon: Info,
    borderColor: 'border-sky-500/40',
    bgColor: 'bg-sky-950/20',
    iconColor: 'text-sky-400',
    defaultTitle: 'Note',
  },
  tip: {
    icon: Sparkles,
    borderColor: 'border-emerald-500/40',
    bgColor: 'bg-emerald-950/20',
    iconColor: 'text-emerald-400',
    defaultTitle: 'Tip',
  },
  important: {
    icon: AlertCircle,
    borderColor: 'border-purple-500/40',
    bgColor: 'bg-purple-950/20',
    iconColor: 'text-purple-400',
    defaultTitle: 'Important',
  },
  warning: {
    icon: AlertTriangle,
    borderColor: 'border-amber-500/40',
    bgColor: 'bg-amber-950/20',
    iconColor: 'text-amber-400',
    defaultTitle: 'Warning',
  },
  danger: {
    icon: AlertTriangle,
    borderColor: 'border-red-500/40',
    bgColor: 'bg-red-950/20',
    iconColor: 'text-red-400',
    defaultTitle: 'Danger',
  },
  security: {
    icon: ShieldAlert,
    borderColor: 'border-rose-500/40',
    bgColor: 'bg-rose-950/20',
    iconColor: 'text-rose-400',
    defaultTitle: 'Security Boundary',
  },
  invariant: {
    icon: Lock,
    borderColor: 'border-cyan-500/50',
    bgColor: 'bg-cyan-950/25',
    iconColor: 'text-cyan-400',
    defaultTitle: 'Protocol Invariant',
  },
};

export function Callout({ type = 'note', title, children }: CalloutProps) {
  const config = CALLOUT_CONFIG[type] || CALLOUT_CONFIG.note;
  const Icon = config.icon;

  return (
    <div
      className={`my-5 p-4 rounded-xl border ${config.borderColor} ${config.bgColor} backdrop-blur-md shadow-card transition-all`}
    >
      <div className="flex items-start gap-3">
        <div className="p-1 rounded-lg bg-bg-surface/60 border border-border-subtle mt-0.5 shrink-0">
          <Icon className={`w-4 h-4 ${config.iconColor}`} />
        </div>
        <div className="space-y-1 text-sm leading-relaxed text-text-primary">
          <div className="font-semibold tracking-tight text-text-primary flex items-center gap-2">
            <span>{title || config.defaultTitle}</span>
          </div>
          <div className="text-text-secondary text-sm prose-p:my-1">{children}</div>
        </div>
      </div>
    </div>
  );
}
