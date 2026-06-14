import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from "@/components/ui/accordion";

export default function FAQPage() {
  return (
    <div className="space-y-6">
      <div className="relative rounded-2xl overflow-hidden mb-10 border border-blue-500/10 bg-gradient-to-r from-blue-950/20 via-zinc-900 to-zinc-950 p-8 shadow-lg">
        <div className="absolute top-0 right-0 w-32 h-32 bg-blue-500/10 blur-2xl rounded-full"></div>
        <span className="text-xs font-semibold uppercase tracking-wider text-blue-400 mb-2 block">Help Center</span>
        <h1 className="text-3xl font-bold tracking-tight text-white m-0 font-sans">Frequently Asked Questions</h1>
        <p className="text-zinc-400 text-sm mt-2 max-w-xl">
          Answers to common architectural, security, and design questions regarding the ZAP protocol.
        </p>
      </div>

      <Accordion className="w-full space-y-4">
        <AccordionItem value="item-1" className="border border-zinc-800 bg-zinc-950/30 rounded-xl px-4">
          <AccordionTrigger className="text-white hover:no-underline text-base font-semibold py-4">
            How does ZAP compare to MQTT or gRPC?
          </AccordionTrigger>
          <AccordionContent className="text-zinc-400 text-sm pb-4 leading-relaxed">
            <p className="mb-2"><strong>MQTT</strong> relies on a central, single-point-of-failure broker, uses TCP (susceptible to head-of-line blocking), and lacks native payload signatures. ZAP is brokerless, running peer-to-peer over UDP with end-to-end cryptographic signatures and sandboxed WebAssembly execution.</p>
            <p><strong>gRPC</strong> runs over HTTP/2 (TCP) with heavy connection and serialization overhead. It lacks native multi-node consensus gates or sandboxed driver runtimes at the protocol layer.</p>
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="item-2" className="border border-zinc-800 bg-zinc-950/30 rounded-xl px-4">
          <AccordionTrigger className="text-white hover:no-underline text-base font-semibold py-4">
            Does ZAP require a central coordinator server?
          </AccordionTrigger>
          <AccordionContent className="text-zinc-400 text-sm pb-4 leading-relaxed">
            No. ZAP is a fully peer-to-peer (P2P) protocol. Nodes discover and communicate directly with each other using local, auditable peer configuration tables. There is no central registry, discovery server, or coordinator needed.
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="item-3" className="border border-zinc-800 bg-zinc-950/30 rounded-xl px-4">
          <AccordionTrigger className="text-white hover:no-underline text-base font-semibold py-4">
            What is the maximum payload size?
          </AccordionTrigger>
          <AccordionContent className="text-zinc-400 text-sm pb-4 leading-relaxed">
            The ZAP-Wire specification defines a maximum payload length of 16 MB. However, because ZAP uses UDP, large payloads will be fragmented. For low-latency edge applications, keeping payloads under the MTU (~1400 bytes) is highly recommended.
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="item-4" className="border border-zinc-800 bg-zinc-950/30 rounded-xl px-4">
          <AccordionTrigger className="text-white hover:no-underline text-base font-semibold py-4">
            Why Ed25519 signatures and what is ZAP_SIGN?
          </AccordionTrigger>
          <AccordionContent className="text-zinc-400 text-sm pb-4 leading-relaxed">
            Ed25519 signatures are compact, extremely fast, and highly secure. Because full verification is CPU-intensive, ZAP includes an 8-byte <code>ZAP_SIGN</code> hint (a BLAKE3 signature hash) in the header. Nodes check the hint first to drop invalid or DoS traffic instantly, avoiding heavy signature verification math for invalid packets.
          </AccordionContent>
        </AccordionItem>

        <AccordionItem value="item-5" className="border border-zinc-800 bg-zinc-950/30 rounded-xl px-4">
          <AccordionTrigger className="text-white hover:no-underline text-base font-semibold py-4">
            Is Proof-of-Action (PoA) a blockchain?
          </AccordionTrigger>
          <AccordionContent className="text-zinc-400 text-sm pb-4 leading-relaxed">
            No. ZAP does not use blocks, proof-of-work, or proof-of-stake. PoA is a lightweight threshold validator scheme. It simply ensures that high-risk actions carry cryptographic approval signatures (attestations) from a validator quorum before a node executes them.
          </AccordionContent>
        </AccordionItem>
      </Accordion>
    </div>
  );
}
