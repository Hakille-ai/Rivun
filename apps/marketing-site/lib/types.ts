export type MessageKindName =
  | "data"
  | "event"
  | "command"
  | "query"
  | "response"
  | "streamChunk"
  | "action"
  | "control";

export interface MessageKindInfo {
  id: number;
  name: MessageKindName;
  label: string;
  description: string;
}

export const MESSAGE_KINDS: Record<MessageKindName, MessageKindInfo> = {
  data: { id: 1, name: "data", label: "Data (0x01)", description: "Raw sensor reading or high-throughput telemetry stream" },
  event: { id: 2, name: "event", label: "Event (0x02)", description: "Publish-subscribe broadcast event across the mesh" },
  command: { id: 3, name: "command", label: "Command (0x03)", description: "Direct actuation instruction targeting a machine or subsystem" },
  query: { id: 4, name: "query", label: "Query (0x04)", description: "Idempotent read-only state query across nodes" },
  response: { id: 5, name: "response", label: "Response (0x05)", description: "Cryptographically verified response to command or query" },
  streamChunk: { id: 6, name: "streamChunk", label: "StreamChunk (0x06)", description: "Flow-controlled frame within a zero-copy SPSC stream" },
  action: { id: 7, name: "action", label: "Action (0x07)", description: "High-level agent intent targeting sandboxed WASM driver" },
  control: { id: 8, name: "control", label: "Control (0x08)", description: "Mesh coordination, BFT consensus votes, or gossip heartbeat" },
};

export interface ProtocolFlags {
  encrypted: boolean;        // 0x0001
  priority: boolean;         // 0x0002
  requiresConsensus: boolean;// 0x0004
  signed: boolean;           // 0x0008
  broadcast: boolean;        // 0x0010
}

export interface ByteSegment {
  name: string;
  category: "magic" | "version" | "flags" | "source" | "target" | "timestamp" | "length" | "hint" | "envelope" | "subject" | "content_type" | "payload" | "auth_trailer" | "poa_trailer";
  offset: number;
  length: number;
  hex: string;
  description: string;
  parsedValue: string;
  colorClass: string;
}

export interface EncodedFrameResult {
  rawBytes: Uint8Array;
  totalSize: number;
  wireHeaderSize: number;
  envelopeHeaderSize: number;
  payloadSize: number;
  authTrailerSize: number;
  poaTrailerSize: number;
  segments: ByteSegment[];
  hexDumpLines: HexDumpLine[];
  blake3Digest: string;
  signatureHint: string;
  sourceUuid: string;
  targetUuid: string;
  timestampMicros: bigint;
}

export interface HexDumpLine {
  offset: number;
  offsetHex: string;
  hexBytes: Array<{ byteHex: string; globalOffset: number; segmentIndex: number; colorClass: string }>;
  ascii: string;
}

export type RiskLevel = "low" | "medium" | "high" | "critical";

export interface DomainPackCapability {
  name: string;
  description: string;
  risk: RiskLevel;
  requiredProof: string;
}

export interface DomainPackInfo {
  id: string;
  name: string;
  tagline: string;
  category: "ai" | "cloud" | "enterprise" | "iot";
  version: string;
  capabilitiesCount: number;
  defaultSafetyGate: string;
  description: string;
  manifestToml: string;
  policyToml: string;
  schemaJson: string;
  capabilities: DomainPackCapability[];
}

export interface PricingTier {
  id: string;
  name: string;
  badge?: string;
  priceMonthly: number;
  priceAnnual: number;
  description: string;
  features: string[];
  ctaLabel: string;
  popular?: boolean;
}
