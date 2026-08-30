import type { Metadata } from 'next';
import './globals.css';
import React from 'react';

export const metadata: Metadata = {
  title: 'Rivun Documentation Portal — Zero-Trust Autonomous Protocol',
  description:
    'Official developer documentation, crate API reference, SDK manuals, and protocol specifications for the Rivun (ZAP) ecosystem.',
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="bg-bg-base text-text-primary min-h-screen antialiased selection:bg-cyan-500/30 selection:text-cyan-200">
        {children}
      </body>
    </html>
  );
}
