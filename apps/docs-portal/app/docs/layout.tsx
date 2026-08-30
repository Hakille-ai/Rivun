'use client';

import React, { useState } from 'react';
import { Header } from '@/components/layout/Header';
import { Sidebar } from '@/components/layout/Sidebar';
import { SearchModal } from '@/components/ui/SearchModal';
import { Footer } from '@/components/layout/Footer';

export default function DocsLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  const [isSearchOpen, setIsSearchOpen] = useState(false);
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  return (
    <div className="min-h-screen flex flex-col bg-bg-base">
      <Header
        onOpenSearch={() => setIsSearchOpen(true)}
        onToggleSidebar={() => setIsSidebarOpen((prev) => !prev)}
        isSidebarOpen={isSidebarOpen}
      />
      <SearchModal isOpen={isSearchOpen} onClose={() => setIsSearchOpen(false)} />

      {/* Main Layout Body */}
      <div className="flex-1 max-w-7xl w-full mx-auto flex">
        {/* Desktop Left Sidebar (sticky) */}
        <div className="hidden lg:block w-72 shrink-0 border-r border-border-subtle sticky top-16 h-[calc(100vh-4rem)]">
          <Sidebar />
        </div>

        {/* Mobile Left Sidebar Drawer */}
        {isSidebarOpen && (
          <div className="lg:hidden fixed inset-0 z-50 flex">
            <div
              className="fixed inset-0 bg-black/70 backdrop-blur-sm"
              onClick={() => setIsSidebarOpen(false)}
            />
            <div className="relative w-80 max-w-[85vw] h-full bg-bg-surface z-10 shadow-modal">
              <Sidebar onCloseMobile={() => setIsSidebarOpen(false)} />
            </div>
          </div>
        )}

        {/* Center Content & Right TOC */}
        <main className="flex-1 min-w-0 px-4 sm:px-8 py-8">
          {children}
        </main>
      </div>

      <Footer />
    </div>
  );
}
