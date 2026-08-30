'use client';

import React, { useState } from 'react';
import { PlayCircle, Server, ShieldCheck, Terminal } from 'lucide-react';
import { Header } from '@/components/layout/Header';
import { Footer } from '@/components/layout/Footer';
import { SearchModal } from '@/components/ui/SearchModal';
import { ApiRequestTester } from '@/components/interactive/ApiRequestTester';
import { Badge } from '@/components/ui/Badge';

export default function ApiExplorerPage() {
  const [isSearchOpen, setIsSearchOpen] = useState(false);

  return (
    <div className="min-h-screen flex flex-col bg-bg-base">
      <Header onOpenSearch={() => setIsSearchOpen(true)} />
      <SearchModal isOpen={isSearchOpen} onClose={() => setIsSearchOpen(false)} />

      <main className="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-10">
        {/* Page Header */}
        <div className="mb-8">
          <div className="flex items-center gap-2 mb-2">
            <Badge variant="cyan">Rivun Cloud SaaS Control Plane</Badge>
            <Badge variant="outline">Axum 0.8 REST & SSE</Badge>
          </div>
          <h1 className="text-3xl font-extrabold tracking-tight text-text-primary">
            Rivun Cloud REST API Live Explorer
          </h1>
          <p className="mt-2 text-sm text-text-secondary max-w-3xl">
            Test and inspect multi-tenant Rivun Cloud API endpoints in real-time. Verify cluster status,
            query fleet node inventories, retrieve signed action receipts from the MMR ledger, and stage zero-trust policy bundles.
          </p>
        </div>

        {/* Interactive REST Runner */}
        <ApiRequestTester />
      </main>

      <Footer />
    </div>
  );
}
