'use client';

import React, { useState } from 'react';
import { Play, Check, Copy, Server, Globe, CornerDownRight } from 'lucide-react';
import { Badge } from '@/components/ui/Badge';

interface EndpointConfig {
  id: string;
  name: string;
  method: 'GET' | 'POST';
  path: string;
  defaultBody?: string;
  mockResponse: unknown;
}

const ENDPOINTS: EndpointConfig[] = [
  {
    id: 'status',
    name: 'Get Cluster & Cloud Status',
    method: 'GET',
    path: '/v1/status',
    mockResponse: {
      status: 'HEALTHY',
      version: '1.0.0',
      nodes_active: 8,
      consensus_quorum: 'T=6 of N=8',
      mmr_peak_count: 3,
      uptime_seconds: 418290,
      protocol: 'ZAP_v1',
    },
  },
  {
    id: 'nodes',
    name: 'List Fleet Nodes',
    method: 'GET',
    path: '/v1/orgs/org-engineering-01/nodes',
    mockResponse: {
      nodes: [
        {
          node_id: 'd8f1e09a-4c22-4819-bf91-30912384a101',
          role: 'LEADER',
          addr: '10.0.1.15:9001',
          status: 'HEALTHY',
          rtt_ms: 1.2,
          last_heartbeat: 1787884800,
        },
        {
          node_id: 'a13c907b-8910-412e-9d21-998811223344',
          role: 'VALIDATOR',
          addr: '10.0.1.16:9001',
          status: 'HEALTHY',
          rtt_ms: 2.4,
          last_heartbeat: 1787884800,
        },
      ],
      total: 2,
    },
  },
  {
    id: 'receipts',
    name: 'List Signed Action Receipts',
    method: 'GET',
    path: '/v1/orgs/org-engineering-01/receipts?limit=5',
    mockResponse: {
      receipts: [
        {
          receipt_hash: '9f8e7d6c5b4a3a2b1c0de1f2a3b4c5d6e7f80918273645a4b3c2d1e0f9a8b7c6',
          subject: 'scada.hvac.temperature.set',
          initiator_node: 'd8f1e09a-4c22-4819-bf91-30912384a101',
          mmr_leaf_index: 4092,
          poa_attestations: 3,
          timestamp_micros: 1787884799500000,
          verified: true,
        },
      ],
    },
  },
  {
    id: 'stage_policy',
    name: 'Stage Zero-Trust Policy Bundle',
    method: 'POST',
    path: '/v1/orgs/org-engineering-01/policies/stage',
    defaultBody: JSON.stringify(
      {
        policy_name: 'prod-datacenter-safety-v2',
        risk_tier: 'HIGH',
        rules: [
          {
            subject: 'scada.hvac.*',
            effect: 'REQUIRE_POA',
            threshold: 3,
          },
        ],
      },
      null,
      2
    ),
    mockResponse: {
      staging_id: 'stage-7b91a03e-4029-41a2',
      status: 'STAGED',
      signature_required: true,
      domain_separator: 'Rivun-POLICY-BUNDLE-v1',
      expires_at: 1787888400,
      instructions: 'Review policy diff and sign with rivun-control operator key before deployment.',
    },
  },
  {
    id: 'packs',
    name: 'List RivunStore Domain Packs',
    method: 'GET',
    path: '/v1/registry/packs',
    mockResponse: {
      packs: [
        {
          name: 'rivun-pack-agentic-dev',
          version: '1.0.0',
          author: 'Rivun Security Foundation',
          risk_tier: 'MEDIUM',
          drivers_count: 4,
          verified_signature: true,
        },
        {
          name: 'rivun-pack-smart-building',
          version: '1.2.0',
          author: 'EdgeIoT Systems',
          risk_tier: 'HIGH',
          drivers_count: 6,
          verified_signature: true,
        },
      ],
    },
  },
];

export function ApiRequestTester() {
  const [selectedId, setSelectedId] = useState<string>('status');
  const [bodyText, setBodyText] = useState<string>('');
  const [response, setResponse] = useState<unknown | null>(null);
  const [loading, setLoading] = useState(false);
  const [latency, setLatency] = useState<number | null>(null);

  const activeEndpoint = ENDPOINTS.find((e) => e.id === selectedId) || ENDPOINTS[0];

  const handleSelectEndpoint = (id: string) => {
    setSelectedId(id);
    const ep = ENDPOINTS.find((e) => e.id === id);
    if (ep && ep.defaultBody) {
      setBodyText(ep.defaultBody);
    } else {
      setBodyText('');
    }
    setResponse(null);
    setLatency(null);
  };

  const handleExecute = () => {
    setLoading(true);
    setResponse(null);
    setTimeout(() => {
      setLoading(false);
      setResponse(activeEndpoint.mockResponse);
      setLatency(Math.floor(Math.random() * 8) + 9); // 9-16ms
    }, 250);
  };

  return (
    <div className="space-y-6">
      <div className="p-6 rounded-2xl border border-border-subtle bg-bg-surface shadow-card">
        <div className="flex items-center justify-between pb-4 mb-5 border-b border-border-subtle">
          <div className="flex items-center gap-2">
            <Server className="w-5 h-5 text-accent-primary" />
            <h3 className="text-base font-bold text-text-primary">
              Rivun Cloud REST API Live Explorer
            </h3>
          </div>
          <Badge variant="cyan">Axum 0.8 REST / SSE Server</Badge>
        </div>

        {/* Endpoint Selector Tabs */}
        <div className="flex items-center gap-2 overflow-x-auto pb-2 mb-4">
          {ENDPOINTS.map((ep) => (
            <button
              key={ep.id}
              onClick={() => handleSelectEndpoint(ep.id)}
              className={`flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-mono transition-all whitespace-nowrap border ${
                selectedId === ep.id
                  ? 'bg-accent-primary/10 border-accent-primary text-text-primary font-bold shadow-glow'
                  : 'bg-bg-subtle border-border-subtle text-text-secondary hover:text-text-primary hover:border-border-strong'
              }`}
            >
              <Badge variant={ep.method === 'GET' ? 'cyan' : 'emerald'}>
                {ep.method}
              </Badge>
              <span>{ep.name}</span>
            </button>
          ))}
        </div>

        {/* Address Bar & Send Button */}
        <div className="flex items-center gap-2 p-2 rounded-xl bg-bg-subtle border border-border-subtle mb-6">
          <Badge variant={activeEndpoint.method === 'GET' ? 'cyan' : 'emerald'}>
            {activeEndpoint.method}
          </Badge>
          <div className="flex-1 font-mono text-xs text-cyan-300 truncate">
            https://api.rivun.cloud{activeEndpoint.path}
          </div>
          <button
            onClick={handleExecute}
            disabled={loading}
            className="flex items-center gap-1.5 px-4 py-2 rounded-lg bg-accent-primary hover:bg-sky-400 text-bg-base font-bold text-xs transition-all shadow-glow disabled:opacity-50"
          >
            <Play className="w-3.5 h-3.5 fill-current" />
            <span>{loading ? 'Executing...' : 'Send Request'}</span>
          </button>
        </div>

        {/* Request Body (if POST) */}
        {activeEndpoint.method === 'POST' && (
          <div className="mb-6 space-y-1.5">
            <label className="text-xs font-semibold text-text-primary">
              Request Payload (application/json)
            </label>
            <textarea
              rows={6}
              value={bodyText || activeEndpoint.defaultBody || ''}
              onChange={(e) => setBodyText(e.target.value)}
              className="w-full p-3 rounded-xl bg-[#080B10] border border-border-subtle font-mono text-xs text-text-primary focus:outline-none focus:border-accent-primary"
            />
          </div>
        )}

        {/* Response Panel */}
        {response ? (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-2">
                <span className="text-xs font-bold text-text-primary uppercase tracking-wider">
                  Response
                </span>
                <Badge variant="emerald">200 OK</Badge>
                {latency && (
                  <span className="text-[11px] font-mono text-text-muted">
                    Latency: {latency} ms
                  </span>
                )}
              </div>
            </div>

            <div className="p-4 rounded-xl bg-[#080B10] border border-emerald-500/30 font-mono text-xs text-emerald-300 overflow-x-auto shadow-card">
              <pre>{JSON.stringify(response, null, 2)}</pre>
            </div>
          </div>
        ) : (
          <div className="p-8 rounded-xl bg-bg-subtle/40 border border-border-subtle text-center text-xs text-text-muted">
            Click &ldquo;Send Request&rdquo; to execute this call against the Rivun Cloud mock runtime.
          </div>
        )}
      </div>
    </div>
  );
}
