import { Download, Terminal, KeyRound, Container, ArrowRight } from 'lucide-react';
import Link from 'next/link';
import { Badge } from '@/components/ui/badge';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';

const commands = {
  build: `git clone https://github.com/Hakille-ai/ZAP.git
cd ZAP

cargo test --workspace --all-targets
cargo build --release -p zap-cli`,
  keys: `cargo run -p zap-cli -- keygen --out .zap/node-a.key
cargo run -p zap-cli -- keygen --out .zap/node-b.key`,
  validate: `cargo run -p zap-cli -- check-config --config examples/configs/node-a.toml
cargo run -p zap-cli -- doctor --config examples/configs/node-a.toml`,
  docker: `docker build -t zap:local .

mkdir -p .zap/container
docker compose run --rm node keygen --out /var/lib/zap/node.key
docker compose up --build`,
};

function CodeBlock({ children }: { children: string }) {
  return (
    <pre className="overflow-x-auto rounded-lg border border-zinc-900 bg-[#050505] p-4 text-xs text-zinc-300">
      <code>{children}</code>
    </pre>
  );
}

export default function InstallPage() {
  return (
    <div className="space-y-8 font-sans">
      <div className="space-y-3">
        <Badge className="bg-blue-500/10 text-blue-400 border-blue-500/20">Source install</Badge>
        <h1 className="text-3xl md:text-4xl font-extrabold tracking-tight text-white">Install ZAP</h1>
        <p className="text-zinc-400 text-lg">
          Build the ZAP workspace, compile the operator CLI, and prepare a local node setup from the current repository.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4 not-prose">
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <Download className="w-5 h-5 text-blue-400" />
            <CardTitle className="text-base text-white">Rust 1.93+</CardTitle>
            <CardDescription>Selected through the repo toolchain file when available.</CardDescription>
          </CardHeader>
        </Card>
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <Terminal className="w-5 h-5 text-blue-400" />
            <CardTitle className="text-base text-white">Git and Cargo</CardTitle>
            <CardDescription>Clone, test, and build the `zap-cli` binary locally.</CardDescription>
          </CardHeader>
        </Card>
        <Card className="bg-zinc-950/40 border-zinc-850">
          <CardHeader>
            <Container className="w-5 h-5 text-blue-400" />
            <CardTitle className="text-base text-white">Docker Optional</CardTitle>
            <CardDescription>Use the checked-in Dockerfile and Compose setup.</CardDescription>
          </CardHeader>
        </Card>
      </div>

      <section className="space-y-4">
        <h2 className="text-xl font-bold text-white">Clone and Build</h2>
        <p className="text-zinc-400">
          ZAP is currently installed from source. The release build writes `zap` to `target/release/zap`
          or `target/release/zap.exe` on Windows.
        </p>
        <CodeBlock>{commands.build}</CodeBlock>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-bold text-white">Prepare Local Keys</h2>
        <p className="text-zinc-400">
          Every node needs an Ed25519 identity. `keygen` prints the generated `node_id` and `public_key`;
          copy each public key into the peer entry of the other node config before signed sends.
        </p>
        <CodeBlock>{commands.keys}</CodeBlock>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-bold text-white">Validate Configs</h2>
        <p className="text-zinc-400">
          The files in `examples/configs/` are templates for a local two-node setup. Validate them before binding sockets.
        </p>
        <CodeBlock>{commands.validate}</CodeBlock>
      </section>

      <section className="space-y-4">
        <h2 className="text-xl font-bold text-white">Docker Quickstart</h2>
        <CodeBlock>{commands.docker}</CodeBlock>
      </section>

      <Card className="not-prose bg-zinc-950/40 border-zinc-850">
        <CardContent className="p-5 flex flex-col md:flex-row md:items-center md:justify-between gap-4">
          <div className="flex items-start gap-3">
            <KeyRound className="w-5 h-5 text-blue-400 mt-0.5" />
            <div>
              <h3 className="font-semibold text-white">Next: run a two-node demo</h3>
              <p className="text-sm text-zinc-400">Continue with the full onboarding guide for local send and receive flows.</p>
            </div>
          </div>
          <Link href="/docs/getting-started" className="inline-flex items-center gap-2 text-sm font-medium text-blue-400 hover:text-blue-300">
            Getting Started <ArrowRight className="w-4 h-4" />
          </Link>
        </CardContent>
      </Card>
    </div>
  );
}
