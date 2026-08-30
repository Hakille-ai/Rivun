import React from 'react';
import { Sigma } from 'lucide-react';

interface MathFormulaProps {
  formula: string;
  title?: string;
  explanation?: string;
}

export function MathFormula({ formula, title, explanation }: MathFormulaProps) {
  return (
    <div className="my-5 p-4 rounded-xl border border-cyan-500/30 bg-cyan-950/15 backdrop-blur-md shadow-card">
      <div className="flex items-center justify-between pb-2 mb-2 border-b border-cyan-500/20 text-xs">
        <div className="flex items-center gap-1.5 text-cyan-400 font-semibold">
          <Sigma className="w-4 h-4" />
          <span>{title || 'Mathematical Formulation'}</span>
        </div>
      </div>
      <div className="py-2 overflow-x-auto text-center font-mono text-base text-cyan-200 tracking-wide">
        <code>{formula}</code>
      </div>
      {explanation && (
        <p className="mt-2 text-xs text-text-secondary leading-normal">
          {explanation}
        </p>
      )}
    </div>
  );
}
