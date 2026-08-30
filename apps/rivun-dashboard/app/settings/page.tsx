"use client";

import React, { useEffect, useState } from "react";
import {
  Settings,
  Users,
  Key,
  Shield,
  Clock,
  Plus,
  Trash2,
  CheckCircle2,
  Activity,
  Layers,
} from "lucide-react";
import { api } from "../../lib/api";
import { AuditLogRecord, Membership, UsageCounters, UserRole } from "../../lib/types";

export default function SettingsPage() {
  const [members, setMembers] = useState<Membership[]>([]);
  const [auditLogs, setAuditLogs] = useState<AuditLogRecord[]>([]);
  const [usage, setUsage] = useState<UsageCounters | null>(null);
  const [activeTab, setActiveTab] = useState<"team" | "tokens" | "audit" | "usage">("team");

  // Invite modal state
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteName, setInviteName] = useState("");
  const [inviteRole, setInviteRole] = useState<UserRole>("operator");
  const [isInviting, setIsInviting] = useState(false);
  const [notification, setNotification] = useState<string | null>(null);

  useEffect(() => {
    async function load() {
      const [m, a, u] = await Promise.all([
        api.fetchMembers(),
        api.fetchAuditLog(),
        api.fetchUsage(),
      ]);
      setMembers(m);
      setAuditLogs(a);
      setUsage(u);
    }
    load();
  }, []);

  const handleAddMember = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!inviteEmail) return;
    setIsInviting(true);
    const newMember: Membership = {
      user_id: `u-${Date.now()}`,
      org_id: "a1",
      role: inviteRole,
      user_email: inviteEmail,
      user_name: inviteName || inviteEmail.split("@")[0],
      joined_at: new Date().toISOString(),
    };
    setMembers((prev) => [...prev, newMember]);
    setIsInviting(false);
    setInviteEmail("");
    setInviteName("");
    setNotification(`Invited ${newMember.user_email} as ${inviteRole.toUpperCase()}`);
    setTimeout(() => setNotification(null), 4000);
  };

  return (
    <div className="space-y-8 animate-in fade-in duration-300">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-text-primary tracking-tight">Organization & Security</h1>
          <p className="text-sm text-text-secondary">
            Team access control (RBAC), Bridge API tokens, Meta-Audit trail, and billing usage.
          </p>
        </div>
      </div>

      {notification && (
        <div className="p-3.5 rounded-xl bg-status-verified-bg border border-status-verified/30 text-xs text-status-verified flex items-center space-x-2">
          <CheckCircle2 className="w-4 h-4 shrink-0" />
          <span>{notification}</span>
        </div>
      )}

      {/* Tabs */}
      <div className="flex items-center space-x-2 border-b border-border-subtle pb-3">
        <button
          onClick={() => setActiveTab("team")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "team"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Team & RBAC ({members.length})
        </button>
        <button
          onClick={() => setActiveTab("tokens")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "tokens"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Bridge API Tokens
        </button>
        <button
          onClick={() => setActiveTab("audit")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "audit"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Dashboard Meta-Audit Log
        </button>
        <button
          onClick={() => setActiveTab("usage")}
          className={`px-3.5 py-1.5 rounded-lg text-xs font-semibold transition ${
            activeTab === "usage"
              ? "bg-bg-surface-raised text-accent-primary border border-border-subtle"
              : "text-text-secondary hover:text-text-primary"
          }`}
        >
          Usage & Quotas
        </button>
      </div>

      {activeTab === "team" && (
        <div className="space-y-6">
          {/* Invite Form */}
          <div className="p-6 rounded-2xl bg-bg-surface border border-border-subtle space-y-4">
            <h3 className="text-sm font-semibold text-text-primary uppercase tracking-wide font-mono">
              Invite Team Member
            </h3>
            <form onSubmit={handleAddMember} className="grid grid-cols-1 sm:grid-cols-4 gap-4">
              <input
                type="text"
                placeholder="Full Name"
                value={inviteName}
                onChange={(e: any) => setInviteName(e.target.value)}
                className="px-3 py-2 rounded-lg bg-bg-surface-raised border border-border-subtle text-xs text-text-primary focus:outline-none focus:border-accent-primary"
              />
              <input
                type="email"
                placeholder="operator@company.com"
                required
                value={inviteEmail}
                onChange={(e: any) => setInviteEmail(e.target.value)}
                className="px-3 py-2 rounded-lg bg-bg-surface-raised border border-border-subtle text-xs text-text-primary focus:outline-none focus:border-accent-primary"
              />
              <select
                value={inviteRole}
                onChange={(e: any) => setInviteRole(e.target.value as UserRole)}
                className="px-3 py-2 rounded-lg bg-bg-surface-raised border border-border-subtle text-xs text-text-primary focus:outline-none focus:border-accent-primary"
              >
                <option value="operator">Operator (Can deploy & sign)</option>
                <option value="auditor">Auditor (Read-only ledger access)</option>
                <option value="admin">Admin (Manage settings)</option>
                <option value="owner">Owner (Full access)</option>
              </select>
              <button
                type="submit"
                disabled={isInviting}
                className="px-4 py-2 rounded-lg bg-accent-primary hover:bg-accent-hover text-white text-xs font-semibold shadow-glow transition flex items-center justify-center space-x-1.5"
              >
                <Plus className="w-3.5 h-3.5" />
                <span>Add Member</span>
              </button>
            </form>
          </div>

          {/* Members Table */}
          <div className="rounded-2xl bg-bg-surface border border-border-subtle overflow-hidden">
            <table className="w-full text-left border-collapse text-xs">
              <thead>
                <tr className="border-b border-border-subtle bg-bg-surface-raised text-text-secondary font-mono uppercase tracking-wider text-[10px]">
                  <th className="py-3 px-4">Member Name</th>
                  <th className="py-3 px-4">Email</th>
                  <th className="py-3 px-4">Role / Permissions</th>
                  <th className="py-3 px-4">Joined Date</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border-subtle">
                {members.map((m) => (
                  <tr key={m.user_id} className="hover:bg-bg-surface-raised transition">
                    <td className="py-3.5 px-4 font-semibold text-text-primary">{m.user_name}</td>
                    <td className="py-3.5 px-4 font-mono text-text-secondary">{m.user_email}</td>
                    <td className="py-3.5 px-4">
                      <span
                        className={`text-[10px] font-mono px-2.5 py-0.5 rounded-full uppercase font-medium ${
                          m.role === "owner"
                            ? "bg-accent-glow text-accent-primary border border-accent-primary/30"
                            : m.role === "auditor"
                            ? "bg-purple-500/10 text-purple-400 border border-purple-500/20"
                            : "bg-bg-surface-raised text-text-secondary border border-border-subtle"
                        }`}
                      >
                        {m.role}
                      </span>
                    </td>
                    <td className="py-3.5 px-4 font-mono text-[11px] text-text-secondary">
                      {new Date(m.joined_at).toLocaleDateString()}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {activeTab === "tokens" && (
        <div className="p-6 rounded-2xl bg-bg-surface border border-border-subtle space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-semibold text-text-primary uppercase tracking-wide font-mono">
              Active Bridge API Tokens
            </h3>
            <button className="px-3 py-1.5 rounded-lg bg-accent-primary text-white text-xs font-medium hover:bg-accent-hover transition shadow-glow flex items-center space-x-1">
              <Plus className="w-3.5 h-3.5" />
              <span>Generate Bridge Token</span>
            </button>
          </div>

          <div className="p-4 rounded-xl bg-bg-surface-raised border border-border-subtle flex items-center justify-between">
            <div className="space-y-1">
              <div className="font-semibold text-text-primary text-xs flex items-center space-x-2">
                <span>Production Edge Bridge Fleet</span>
                <span className="text-[10px] font-mono px-2 py-0.5 rounded bg-status-verified-bg text-status-verified">
                  Active
                </span>
              </div>
              <div className="font-mono text-[11px] text-text-secondary">
                Scopes: <strong className="text-accent-primary font-mono">ingest:write, policies:read</strong>
              </div>
            </div>
            <div className="font-mono text-xs text-text-muted">t00000...0001</div>
          </div>
        </div>
      )}

      {activeTab === "audit" && (
        <div className="rounded-2xl bg-bg-surface border border-border-subtle overflow-hidden">
          <div className="p-4 border-b border-border-subtle text-xs font-semibold text-text-primary uppercase font-mono tracking-wider">
            Immutable Human Action Log (Dashboard Meta-Audit)
          </div>
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="border-b border-border-subtle bg-bg-surface-raised text-text-secondary font-mono uppercase tracking-wider text-[10px]">
                <th className="py-3 px-4">Timestamp</th>
                <th className="py-3 px-4">Actor</th>
                <th className="py-3 px-4">Action</th>
                <th className="py-3 px-4">Target Resource</th>
                <th className="py-3 px-4">Metadata</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-border-subtle">
              {auditLogs.map((log) => (
                <tr key={log.id} className="hover:bg-bg-surface-raised transition font-mono text-[11px]">
                  <td className="py-3 px-4 text-text-secondary">{new Date(log.created_at).toLocaleTimeString()}</td>
                  <td className="py-3 px-4 font-semibold text-text-primary">{log.actor_email}</td>
                  <td className="py-3 px-4 text-accent-primary">{log.action}</td>
                  <td className="py-3 px-4 text-text-primary">{log.target}</td>
                  <td className="py-3 px-4 text-text-muted">{JSON.stringify(log.details)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {activeTab === "usage" && usage && (
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
          <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
            <div className="text-xs font-mono uppercase text-text-secondary mb-2">Ingested Receipts (MTD)</div>
            <div className="text-2xl font-bold text-text-primary font-mono">
              {usage.receipts_ingested.toLocaleString()}
            </div>
            <div className="text-[11px] text-text-secondary mt-1">Quota: 1,000,000 / month</div>
          </div>

          <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
            <div className="text-xs font-mono uppercase text-text-secondary mb-2">Active Fleet Nodes</div>
            <div className="text-2xl font-bold text-text-primary font-mono">{usage.active_nodes}</div>
            <div className="text-[11px] text-text-secondary mt-1">Plan Limit: 50 nodes</div>
          </div>

          <div className="p-5 rounded-2xl bg-bg-surface border border-border-subtle">
            <div className="text-xs font-mono uppercase text-text-secondary mb-2">Policies Deployed</div>
            <div className="text-2xl font-bold text-status-verified font-mono">{usage.policies_deployed}</div>
            <div className="text-[11px] text-text-secondary mt-1">All cryptographically signed</div>
          </div>
        </div>
      )}
    </div>
  );
}
