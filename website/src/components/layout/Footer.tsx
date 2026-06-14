import Link from 'next/link';
import { Terminal } from 'lucide-react';

export default function Footer() {
  return (
    <footer className="border-t border-white/5 bg-[#050505] pt-16 pb-8 mt-24">
      <div className="max-w-7xl mx-auto px-6">
        <div className="grid grid-cols-1 md:grid-cols-4 gap-12 mb-16">
          <div className="col-span-1 md:col-span-2">
            <Link href="/" className="flex items-center gap-2 text-white font-bold tracking-tight text-lg mb-4">
              <Terminal className="w-5 h-5 text-blue-500" />
              <span>ZAP</span>
            </Link>
            <p className="text-zinc-500 text-sm max-w-sm">
              The Universal Low-Latency Protocol for Typed Message Dispatch. 
              Built for AI agents, robotics, industrial control, and zero-trust edge environments.
            </p>
          </div>
          <div>
            <h3 className="text-white font-semibold mb-4">Resources</h3>
            <ul className="space-y-3 text-sm text-zinc-500">
              <li><Link href="/docs" className="hover:text-white transition-colors">Documentation</Link></li>
              <li><Link href="/docs/getting-started" className="hover:text-white transition-colors">Getting Started</Link></li>
              <li><Link href="/docs/tutorial" className="hover:text-white transition-colors">Tutorial</Link></li>
              <li><Link href="/vision" className="hover:text-white transition-colors">Vision</Link></li>
            </ul>
          </div>
          <div>
            <h3 className="text-white font-semibold mb-4">Project</h3>
            <ul className="space-y-3 text-sm text-zinc-500">
              <li><Link href="https://github.com/Hakille-ai/ZAP" className="hover:text-white transition-colors">GitHub</Link></li>
              <li><Link href="https://github.com/Hakille-ai/ZAP/releases" className="hover:text-white transition-colors">Releases</Link></li>
              <li><Link href="https://github.com/Hakille-ai/ZAP/blob/main/LICENSE" className="hover:text-white transition-colors">License</Link></li>
            </ul>
          </div>
        </div>
        <div className="pt-8 border-t border-white/5 flex flex-col md:flex-row justify-between items-center gap-4 text-xs text-zinc-600">
          <p>© {new Date().getFullYear()} Hakille AI. Built with Rust & Next.js.</p>
          <div className="flex gap-4">
            <span>Pre-1.0 Alpha</span>
          </div>
        </div>
      </div>
    </footer>
  );
}
