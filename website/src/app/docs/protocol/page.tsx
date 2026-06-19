import Image from 'next/image';
import { Terminal, Shield, Lock, FileSignature } from 'lucide-react';
import { Card, CardHeader, CardTitle, CardContent, CardDescription } from "@/components/ui/card";
import { Alert, AlertTitle, AlertDescription } from "@/components/ui/alert";
import { Table, TableHeader, TableBody, TableRow, TableCell, TableHead } from "@/components/ui/table";
import { Badge } from "@/components/ui/badge";

export default function ProtocolPage() {
  return (
    <div className="space-y-8 font-sans">
      <div>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight mb-2 text-white">Protocol Specifications</h1>
        <p className="text-zinc-400 text-lg">Deep dive into the byte-level layout of the ZAP wire format.</p>
      </div>

      {/* Hero Visual Card */}
      <Card className="group bg-zinc-950/40 hover:bg-zinc-950/60 border border-zinc-850 hover:border-blue-500/20 transition-all duration-500 overflow-hidden rounded-2xl shadow-xl hover:shadow-2xl hover:shadow-blue-500/5 not-prose p-0">
        <div className="grid grid-cols-1 md:grid-cols-12 items-stretch">
          {/* Image Column */}
          <div className="md:col-span-5 relative bg-gradient-to-br from-zinc-950 to-black/80 flex items-center justify-center p-6 border-b md:border-b-0 md:border-r border-zinc-900 overflow-hidden min-h-[260px] md:min-h-0">
            {/* Ambient Glow Effects */}
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(6,182,212,0.08)_0%,transparent_70%)] pointer-events-none" />
            <div className="absolute -inset-10 bg-[radial-gradient(circle_at_bottom_left,rgba(59,130,246,0.04)_0%,transparent_50%)] pointer-events-none" />
            
            {/* Image Wrapper */}
            <div className="relative w-full aspect-square max-w-[220px] md:max-w-[240px] transform group-hover:scale-[1.03] transition-transform duration-500 ease-out">
              <Image 
                src="/images/zap_packet_layout.png" 
                alt="ZAP Binary Wire Format Visual" 
                fill
                style={{ objectFit: 'contain' }}
                className="opacity-90 group-hover:opacity-100 transition-opacity duration-500"
                priority
              />
            </div>
          </div>
          {/* Content Column */}
          <div className="md:col-span-7 p-6 md:p-8 flex flex-col justify-center space-y-3 bg-zinc-950/20">
            <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 block">Wire Protocol</span>
            <h3 className="text-xl font-bold text-white tracking-tight">Segmented Packet Structure</h3>
            <p className="text-sm text-zinc-400 leading-relaxed">
              ZAP messages consist of a fixed 64-byte Wire Frame header, an optional 74-byte ZENV envelope mapping communication intent, the encrypted data payload, and an optional Proof-of-Action quorum trailer.
            </p>
          </div>
        </div>
      </Card>

      <Alert className="border-blue-500/20 bg-blue-500/5 text-blue-300">
        <Terminal className="w-4 h-4 text-blue-400" />
        <AlertTitle className="font-semibold text-blue-400">Byte-Ordering</AlertTitle>
        <AlertDescription className="text-xs">
          All multi-byte numeric fields in the ZAP Wire Frame header and ZENV envelopes are transmitted in big-endian (network) byte order.
        </AlertDescription>
      </Alert>

      {/* Wire Frame Header Section */}
      <Card className="bg-zinc-950/40 border-zinc-850 overflow-hidden">
        <CardHeader>
          <div className="flex items-center gap-3">
            <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
              <Shield className="w-4 h-4 text-blue-400" />
            </div>
            <div>
              <CardTitle className="text-white text-base">Wire Frame Header (64 bytes)</CardTitle>
              <CardDescription className="text-xs">Fixed-size binary prefix for all network datagrams</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent className="p-0">
          <Table>
            <TableHeader className="bg-zinc-900/40 border-zinc-850">
              <TableRow>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Offset (Bytes)</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Size (Bytes)</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Field</TableHead>
                <TableHead className="px-6 py-3 text-zinc-400 font-semibold text-xs">Description & Magic Values</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody className="divide-y divide-zinc-900">
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">0</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">4</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">MAGIC_NUMBER</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">
                  <Badge variant="outline" className="font-mono bg-zinc-900 text-blue-400 border-zinc-850">0x5A41505F</Badge> (ASCII <code className="text-xs text-white">ZAP_</code>)
                </TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">4</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">2</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">VERSION</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">
                  Currently <Badge variant="outline" className="font-mono bg-zinc-900 text-zinc-400 border-zinc-850">0x0001</Badge> (v1)
                </TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">6</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">2</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">FLAGS</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">
                  Bitmask: Encrypted (0x01), Priority (0x02), Consensus (0x04), Signed (0x08), Broadcast (0x10)
                </TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">8</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">16</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">SOURCE_NODE</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">Cryptographic Node UUID derived from sender&apos;s public identity</TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">24</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">16</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">TARGET_NODE</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">Receiver Node UUID, or all zeroes for broad network propagation</TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">40</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">8</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">TIMESTAMP</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">Unix timestamp in microseconds (prevents out-of-order replay attacks)</TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">48</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">8</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">ZAP_LEN</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">Length of trailing payload segment, up to a hard ceiling of 16 MB</TableCell>
              </TableRow>
              <TableRow className="hover:bg-zinc-950/20">
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">56</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs text-zinc-400">8</TableCell>
                <TableCell className="px-6 py-4 font-mono text-xs white font-medium">ZAP_SIGN</TableCell>
                <TableCell className="px-6 py-4 text-sm text-zinc-300">BLAKE3 signature hash. Checked first to reject fake packets instantly</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </CardContent>
      </Card>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Universal Envelope Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <Lock className="w-4 h-4 text-purple-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Universal Envelope (ZENV)</CardTitle>
                <CardDescription className="text-xs">Dynamic payload header (74 bytes)</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-zinc-400">
            <p>
              When frames carry standard application payloads, they include a ZENV layout prefix mapping specific communication intents:
            </p>
            <ul className="space-y-2 list-disc pl-5 text-zinc-300 text-xs">
              <li><strong>Kinds:</strong> Data (1), Event (2), Command (3), Query (4), Response (5), Stream (6), Action (7), Control (8).</li>
              <li><strong>Causation IDs:</strong> Includes 16-byte Parent ID and 16-byte Correlation ID trackers for swarms.</li>
            </ul>
          </CardContent>
        </Card>

        {/* Proof-of-Action Card */}
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-lg bg-zinc-900 border border-zinc-800 flex items-center justify-center">
                <FileSignature className="w-4 h-4 text-amber-400" />
              </div>
              <div>
                <CardTitle className="text-white text-base">Proof-of-Action Trailer</CardTitle>
                <CardDescription className="text-xs">Consensus verification suffix</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-zinc-400">
            <p>
              If the <code>REQUIRES_CONSENSUS</code> bitmask flag is enabled, a Proof-of-Action trailer is appended directly behind the encrypted payload.
            </p>
            <p className="text-xs text-zinc-300">
              The trailer compiles the frame digest and a packed sequence of validator signatures to guarantee that execution triggers were attested by a threshold majority of nodes.
            </p>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
