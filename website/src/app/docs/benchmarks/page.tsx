"use client";

import { useState } from 'react';
import { Activity, Search, RefreshCw } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import { Input } from "@/components/ui/input";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";

const benchmarks = [
  { name: "capability_advertisement_filter_64", median: "13.47 µs", mean: "13.48 µs", p95: "13.60 µs", samples: 30, cat: "Store & Router", raw: 13467.81 },
  { name: "capability_cache_verify_64_entries", median: "659.40 µs", mean: "661.12 µs", p95: "673.21 µs", samples: 30, cat: "Store & Router", raw: 659402.65 },
  { name: "capability_permissions_to_set", median: "441.64 ns", mean: "442.50 ns", p95: "449.58 ns", samples: 30, cat: "Store & Router", raw: 441.64 },
  { name: "decode_signed_poa_frame", median: "66.60 ns", mean: "66.62 ns", p95: "66.93 ns", samples: 30, cat: "Framing & Core", raw: 66.60 },
  { name: "driver_sdk_execute_trait_echo", median: "13.10 ns", mean: "13.14 ns", p95: "13.44 ns", samples: 30, cat: "WASM Runtime", raw: 13.10 },
  { name: "driver_sdk_pack_unpack_result", median: "1.56 ns", mean: "1.56 ns", p95: "1.56 ns", samples: 30, cat: "WASM Runtime", raw: 1.56 },
  { name: "driver_sdk_packed_result_methods", median: "1.57 ns", mean: "1.57 ns", p95: "1.64 ns", samples: 30, cat: "WASM Runtime", raw: 1.57 },
  { name: "ed25519_sign_frame", median: "20.66 µs", mean: "20.66 µs", p95: "20.72 µs", samples: 30, cat: "Cryptographic", raw: 20660.50 },
  { name: "ed25519_verify_frame", median: "36.38 µs", mean: "36.40 µs", p95: "36.44 µs", samples: 30, cat: "Cryptographic", raw: 36384.10 },
  { name: "encode_frame", median: "24.73 ns", mean: "24.75 ns", p95: "24.78 ns", samples: 30, cat: "Framing & Core", raw: 24.73 },
  { name: "encrypted_udp_round_trip_local", median: "13.08 µs", mean: "14.11 µs", p95: "20.85 µs", samples: 30, cat: "Framing & Core", raw: 13084.96 },
  { name: "ledger_receipt_replication_filter_64", median: "265.63 ns", mean: "265.90 ns", p95: "267.85 ns", samples: 30, cat: "Ledger & Audit", raw: 265.63 },
  { name: "ledger_receipt_replication_response_verify_8", median: "354.47 µs", mean: "354.62 µs", p95: "355.70 µs", samples: 30, cat: "Ledger & Audit", raw: 354471.46 },
  { name: "ledger_sign_action_receipt", median: "44.49 µs", mean: "44.50 µs", p95: "44.59 µs", samples: 30, cat: "Ledger & Audit", raw: 44489.83 },
  { name: "ledger_verify_action_receipt", median: "47.09 µs", mean: "47.39 µs", p95: "51.06 µs", samples: 30, cat: "Ledger & Audit", raw: 47087.74 },
  { name: "memory_query_subject_64_records", median: "211.35 µs", mean: "211.75 µs", p95: "214.15 µs", samples: 30, cat: "Ledger & Audit", raw: 211346.49 },
  { name: "memory_verify_jsonl_64_records", median: "217.49 µs", mean: "218.59 µs", p95: "231.15 µs", samples: 30, cat: "Ledger & Audit", raw: 217489.22 },
  { name: "node_dispatch_zenv_action", median: "154.28 µs", mean: "155.61 µs", p95: "169.48 µs", samples: 30, cat: "Framing & Core", raw: 154283.92 },
  { name: "parse_header_64_bytes", median: "1.71 ns", mean: "1.71 ns", p95: "1.72 ns", samples: 30, cat: "Framing & Core", raw: 1.71 },
  { name: "poa_verify_certificate", median: "36.08 µs", mean: "36.15 µs", p95: "36.37 µs", samples: 30, cat: "Cryptographic", raw: 36078.10 },
  { name: "policy_evaluate_64_rules_last_match", median: "988.90 ns", mean: "965.19 ns", p95: "991.99 ns", samples: 30, cat: "Store & Router", raw: 988.90 },
  { name: "policy_parse_toml_32_rules", median: "54.16 µs", mean: "54.30 µs", p95: "54.82 µs", samples: 30, cat: "Store & Router", raw: 54155.36 },
  { name: "router_decide_64_routes_last_match", median: "866.44 ns", mean: "867.59 ns", p95: "874.42 ns", samples: 30, cat: "Store & Router", raw: 866.44 },
  { name: "router_validate_64_routes", median: "1.47 µs", mean: "1.42 µs", p95: "1.54 µs", samples: 30, cat: "Store & Router", raw: 1472.40 },
  { name: "schema_contract_set_match_32", median: "945.99 ns", mean: "946.42 ns", p95: "950.80 ns", samples: 30, cat: "Store & Router", raw: 945.99 },
  { name: "schema_parse_toml_contract_set_16", median: "82.96 µs", mean: "82.96 µs", p95: "84.49 µs", samples: 30, cat: "Store & Router", raw: 82957.09 },
  { name: "schema_validate_json_contract", median: "514.04 ns", mean: "514.14 ns", p95: "516.12 ns", samples: 30, cat: "Store & Router", raw: 514.04 },
  { name: "store_artifact_hash_4kb", median: "1.59 µs", mean: "1.59 µs", p95: "1.60 µs", samples: 30, cat: "Store & Router", raw: 1593.95 },
  { name: "store_manifest_sign", median: "42.64 µs", mean: "42.76 µs", p95: "43.91 µs", samples: 30, cat: "Store & Router", raw: 42640.69 },
  { name: "store_manifest_verify_driver", median: "43.79 µs", mean: "43.95 µs", p95: "44.21 µs", samples: 30, cat: "Store & Router", raw: 43792.54 },
  { name: "store_publication_verify", median: "119.13 µs", mean: "119.21 µs", p95: "120.11 µs", samples: 30, cat: "Store & Router", raw: 119126.55 },
  { name: "store_registry_hash_16_entries", median: "10.17 µs", mean: "10.18 µs", p95: "10.21 µs", samples: 30, cat: "Store & Router", raw: 10173.89 },
  { name: "store_registry_merge_32_entries", median: "23.53 µs", mean: "23.45 µs", p95: "23.62 µs", samples: 30, cat: "Store & Router", raw: 23525.41 },
  { name: "store_registry_verify_signature_16_entries", median: "60.86 µs", mean: "60.99 µs", p95: "62.50 µs", samples: 30, cat: "Store & Router", raw: 60861.93 },
  { name: "wasm_compile_and_validate_echo", median: "1.28 ms", mean: "1.27 ms", p95: "1.29 ms", samples: 30, cat: "WASM Runtime", raw: 1276906.67 },
  { name: "wasm_execute_echo", median: "40.09 µs", mean: "40.26 µs", p95: "41.65 µs", samples: 30, cat: "WASM Runtime", raw: 40093.36 },
  { name: "zenv_action_encode", median: "48.84 ns", mean: "48.89 ns", p95: "49.53 ns", samples: 30, cat: "Framing & Core", raw: 48.84 },
  { name: "zenv_action_parse", median: "27.80 ns", mean: "27.90 ns", p95: "28.84 ns", samples: 30, cat: "Framing & Core", raw: 27.80 },
];

const categories = ["All", "Framing & Core", "Cryptographic", "WASM Runtime", "Ledger & Audit", "Store & Router"];

export default function BenchmarksPage() {
  const [activeCat, setActiveCat] = useState("All");
  const [search, setSearch] = useState("");

  const filtered = benchmarks.filter(b => {
    const matchesCat = activeCat === "All" || b.cat === activeCat;
    const matchesSearch = b.name.toLowerCase().includes(search.toLowerCase());
    return matchesCat && matchesSearch;
  });

  return (
    <div className="space-y-8">
      {/* Header card */}
      <div className="relative rounded-2xl overflow-hidden border border-blue-500/10 bg-gradient-to-r from-blue-950/20 via-zinc-900 to-zinc-950 p-8 shadow-lg">
        <div className="absolute top-0 right-0 w-32 h-32 bg-blue-500/10 blur-2xl rounded-full"></div>
        <div className="flex items-center gap-3 mb-2">
          <Activity className="w-5 h-5 text-blue-400" />
          <span className="text-xs font-semibold uppercase tracking-wider text-blue-400">Performance Report</span>
        </div>
        <h1 className="text-3xl font-bold tracking-tight text-white m-0 font-sans">ZAP Benchmarks</h1>
        <p className="text-zinc-400 text-sm mt-2 max-w-xl">
          Automated regression benchmarks executing on 64-byte framing targets. Median hot-path processing is strictly constrained under a 7% regression threshold.
        </p>
      </div>

      {/* Metadata stats */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="p-4">
            <span className="text-zinc-500 text-xs block uppercase">Latest Run</span>
            <CardTitle className="text-white font-mono text-sm font-semibold mt-1">main @ epoch 1781360658</CardTitle>
          </CardHeader>
        </Card>
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader className="p-4">
            <span className="text-zinc-500 text-xs block uppercase">Stored Runs</span>
            <CardTitle className="text-white font-mono text-sm font-semibold mt-1">5 runs</CardTitle>
          </CardHeader>
        </Card>
        <Card className="bg-zinc-950/40 border-zinc-850 flex items-center justify-between p-4">
          <div>
            <span className="text-zinc-500 text-xs block uppercase">Criterion Targets</span>
            <CardTitle className="text-white font-mono text-sm font-semibold mt-1">{benchmarks.length} suites</CardTitle>
          </div>
          <RefreshCw className="w-4 h-4 text-zinc-500 animate-spin" />
        </Card>
      </div>

      {/* Controls */}
      <div className="flex flex-col md:flex-row gap-4 items-center justify-between">
        <Tabs defaultValue="All" value={activeCat} onValueChange={setActiveCat} className="w-full md:w-auto">
          <TabsList className="bg-zinc-900 border border-zinc-850 p-1 rounded-xl flex flex-wrap md:flex-nowrap gap-1">
            {categories.map(cat => (
              <TabsTrigger
                key={cat}
                value={cat}
                className="rounded-lg py-1.5 px-3 text-xs font-medium text-zinc-400 data-[state=active]:bg-zinc-950 data-[state=active]:text-white data-[state=active]:shadow-sm"
              >
                {cat}
              </TabsTrigger>
            ))}
          </TabsList>
        </Tabs>
        
        <div className="relative w-full md:w-64">
          <Search className="w-4 h-4 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2" />
          <Input
            type="text"
            placeholder="Search benchmarks..."
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="w-full pl-9 bg-zinc-900 border-zinc-800 text-sm text-white focus:ring-blue-500/30"
          />
        </div>
      </div>

      {/* Table Card */}
      <Card className="bg-zinc-950/40 border-zinc-850 rounded-2xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="border-b border-zinc-850 bg-zinc-950/40">
                <th className="p-4 text-xs font-semibold text-zinc-400">Benchmark</th>
                <th className="p-4 text-xs font-semibold text-zinc-400">Category</th>
                <th className="p-4 text-xs font-semibold text-zinc-400 text-right">Median</th>
                <th className="p-4 text-xs font-semibold text-zinc-400 text-right">Mean</th>
                <th className="p-4 text-xs font-semibold text-zinc-400 text-right">P95</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-900">
              {filtered.map(b => {
                let badgeStyle = "text-blue-400 bg-blue-500/5 border-blue-500/10";
                if (b.raw >= 1000000) {
                  badgeStyle = "text-orange-400 bg-orange-500/5 border-orange-500/10";
                } else if (b.raw >= 1000) {
                  badgeStyle = "text-purple-400 bg-purple-500/5 border-purple-500/10";
                }

                return (
                  <tr key={b.name} className="hover:bg-zinc-950/30 transition-colors">
                    <td className="p-4 font-mono text-xs text-white font-medium">{b.name}</td>
                    <td className="p-4">
                      <span className="px-2 py-0.5 rounded text-[10px] bg-zinc-900 border border-zinc-850 text-zinc-400 font-medium">
                        {b.cat}
                      </span>
                    </td>
                    <td className="p-4 text-right">
                      <span className={`px-2 py-0.5 rounded font-mono text-xs border ${badgeStyle}`}>
                        {b.median}
                      </span>
                    </td>
                    <td className="p-4 font-mono text-xs text-zinc-400 text-right">{b.mean}</td>
                    <td className="p-4 font-mono text-xs text-zinc-400 text-right">{b.p95}</td>
                  </tr>
                );
              })}
              {filtered.length === 0 && (
                <tr>
                  <td colSpan={5} className="p-8 text-center text-zinc-500 text-sm">
                    No benchmarks matching your criteria.
                  </td>
                </tr>
              )}
            </tbody>
          </table>
        </div>
      </Card>
    </div>
  );
}
