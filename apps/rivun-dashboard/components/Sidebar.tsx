"use client";

import React from "react";
import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  LayoutDashboard,
  Server,
  FileCheck,
  Scale,
  ShieldAlert,
  ShoppingBag,
  AlertTriangle,
  Settings,
  TerminalSquare,
  Lock,
} from "lucide-react";

export function Sidebar() {
  const pathname = usePathname();

  const navItems = [
    { label: "Overview", href: "/", icon: LayoutDashboard },
    { label: "Fleet & Doctor", href: "/fleet", icon: Server, badge: "6 Nodes" },
    { label: "Ledger & Receipts", href: "/ledger", icon: FileCheck },
    { label: "Policy Engine", href: "/policies", icon: Scale, badge: "1 Staged", badgeColor: "warning" },
    { label: "Validators (PoA)", href: "/validators", icon: ShieldAlert },
    { label: "Pack Marketplace", href: "/marketplace", icon: ShoppingBag, badge: "7 Packs" },
    { label: "Incidents", href: "/incidents", icon: AlertTriangle, badge: "1 Active", badgeColor: "warning" },
    { label: "Org & Audit Log", href: "/settings", icon: Settings },
  ];

  return (
    <aside className="w-64 border-r border-border-subtle bg-bg-surface flex flex-col justify-between p-4 shrink-0 min-h-[calc(100vh-4rem)]">
      <div className="space-y-6">
        <div>
          <div className="text-[11px] font-semibold text-text-secondary uppercase tracking-wider px-3 mb-2">
            Platform
          </div>
          <nav className="space-y-1">
            {navItems.map((item) => {
              const Icon = item.icon;
              const isActive = pathname === item.href;
              return (
                <Link
                  key={item.href}
                  href={item.href}
                  className={`flex items-center justify-between px-3 py-2 rounded-lg text-sm transition font-medium ${
                    isActive
                      ? "bg-accent-glow text-accent-primary border border-accent-primary/20"
                      : "text-text-secondary hover:text-text-primary hover:bg-bg-surface-raised"
                  }`}
                >
                  <div className="flex items-center space-x-3">
                    <Icon className={`w-4 h-4 ${isActive ? "text-accent-primary" : "text-text-secondary"}`} />
                    <span>{item.label}</span>
                  </div>
                  {item.badge && (
                    <span
                      className={`text-[10px] font-mono px-2 py-0.5 rounded-full ${
                        item.badgeColor === "warning"
                          ? "bg-status-warning-bg text-status-warning border border-status-warning/20"
                          : "bg-bg-surface-raised text-text-secondary border border-border-subtle"
                      }`}
                    >
                      {item.badge}
                    </span>
                  )}
                </Link>
              );
            })}
          </nav>
        </div>

        {/* Local Operator App Promo */}
        <div className="p-3.5 rounded-xl bg-bg-surface-raised border border-border-subtle">
          <div className="flex items-center space-x-2 text-xs font-semibold text-text-primary mb-1">
            <Lock className="w-3.5 h-3.5 text-accent-primary" />
            <span>Rivun Control</span>
          </div>
          <p className="text-[11px] text-text-secondary leading-relaxed mb-3">
            Operator desktop station with OS Keychain. Holds Ed25519 signing keys securely offline.
          </p>
          <div className="flex items-center justify-between">
            <span className="text-[10px] font-mono text-status-verified">● Local Ready</span>
            <span className="text-[10px] text-accent-primary hover:underline cursor-pointer">Open App &rarr;</span>
          </div>
        </div>
      </div>

      {/* Footer Info */}
      <div className="pt-4 border-t border-border-subtle flex items-center justify-between text-[11px] text-text-secondary">
        <span>ZAP Protocol v0.1.0</span>
        <span className="font-mono text-[10px] text-text-muted">BLAKE3/Ed25519</span>
      </div>
    </aside>
  );
}
