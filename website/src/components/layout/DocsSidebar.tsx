"use client";

import Link from 'next/link';
import { usePathname } from 'next/navigation';
import { BookOpen, Key, Shield, Terminal, Cpu, Activity, Network, Database, Lock, HelpCircle, Milestone, Compass, Server, Archive, Info, Download, Package, Bot, FileCheck2 } from 'lucide-react';
const groups = [
  {
    title: "Getting Started",
    items: [
      { title: "Introduction", href: "/docs", icon: BookOpen },
      { title: "Install", href: "/docs/install", icon: Download },
      { title: "Getting Started", href: "/docs/getting-started", icon: Terminal },
      { title: "End-to-End Tutorial", href: "/docs/tutorial", icon: Compass },
      { title: "Use Cases", href: "/docs/use-cases", icon: Activity },
      { title: "FAQ", href: "/docs/faq", icon: HelpCircle },
    ]
  },
  {
    title: "Core Protocol",
    items: [
      { title: "Protocol Specs", href: "/docs/protocol", icon: Shield },
      { title: "Agent Protocol", href: "/docs/agent-protocol", icon: Bot },
      { title: "Message Policy", href: "/docs/message-policy", icon: Shield },
      { title: "Security Model", href: "/docs/security", icon: Lock },
      { title: "RFC / ZEP Process", href: "/docs/rfc-process", icon: FileCheck2 },
      { title: "Signed Receipts", href: "/docs/receipts", icon: Archive },
      { title: "Benchmarks", href: "/docs/benchmarks", icon: Activity },
      { title: "Roadmap", href: "/docs/roadmap", icon: Milestone },
    ]
  },
  {
    title: "Services & Routing",
    items: [
      { title: "ZapStore & Registries", href: "/docs/zapstore", icon: Key },
      { title: "Domain Packs", href: "/docs/domain-packs", icon: Package },
      { title: "Routing & Capabilities", href: "/docs/routing-memory", icon: Network },
    ]
  },
  {
    title: "Runtime & Sandbox",
    items: [
      { title: "Runtime & WASM", href: "/docs/runtime", icon: Cpu },
      { title: "Driver SDK", href: "/docs/sdk", icon: Database },
    ]
  },
  {
    title: "Operations",
    items: [
      { title: "Deployment", href: "/docs/deployment", icon: Server },
      { title: "Observability", href: "/docs/observability", icon: Activity },
      { title: "CLI Operations", href: "/docs/operations", icon: Terminal },
      { title: "Versioning Policy", href: "/docs/versioning", icon: Info },
    ]
  }
];

export default function DocsSidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-64 flex-shrink-0 border-r border-zinc-900 py-8 pr-6 hidden md:block h-full overflow-y-auto">
      <nav className="space-y-8">
        {groups.map((group) => (
          <div key={group.title}>
            <h4 className="text-xs font-semibold text-zinc-500 uppercase tracking-wider mb-3 px-3">{group.title}</h4>
            <ul className="space-y-1">
              {group.items.map((item) => {
                const isActive = pathname === item.href;
                return (
                  <li key={item.href}>
                    <Link
                      href={item.href}
                      className={`flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-all duration-200 ${
                        isActive
                          ? 'bg-blue-500/10 text-blue-400 font-medium border-l-2 border-blue-500 pl-2.5'
                          : 'text-zinc-400 hover:text-white hover:bg-zinc-900'
                      }`}
                    >
                      <item.icon className={`w-4 h-4 ${isActive ? 'text-blue-400' : 'text-zinc-500'}`} />
                      {item.title}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
    </aside>
  );
}
