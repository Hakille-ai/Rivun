import Link from 'next/link';
import { Terminal } from 'lucide-react';

export default function Navbar() {
  return (
    <header className="fixed top-0 w-full z-50 border-b border-white/5 bg-black/50 backdrop-blur-md">
      <div className="max-w-7xl mx-auto px-6 h-16 flex items-center justify-between">
        <Link href="/" className="flex items-center gap-2 text-white font-bold tracking-tight text-lg">
          <Terminal className="w-5 h-5 text-blue-500" />
          <span>ZAP</span>
        </Link>
        <nav className="hidden md:flex items-center gap-8 text-sm font-medium text-zinc-400">
          <Link href="/docs" className="hover:text-white transition-colors">Documentation</Link>
          <Link href="/vision" className="hover:text-white transition-colors">Vision</Link>
          <Link href="https://github.com/Hakille-ai/ZAP" className="hover:text-white transition-colors">GitHub</Link>
        </nav>
        <div className="flex items-center gap-4">
          <Link href="/docs" className="text-sm font-medium bg-white text-black hover:bg-zinc-200 px-4 py-1.5 rounded-full transition-all">
            Get Started
          </Link>
        </div>
      </div>
    </header>
  );
}
