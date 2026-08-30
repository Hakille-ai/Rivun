import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Rivun | Zero-Trust Execution Fabric for Autonomous AI Agents",
  description:
    "High-performance, low-latency, cryptographically signed-by-default execution fabric for AI agents, industrial edge systems, and BFT consensus automation.",
  keywords: [
    "Rivun",
    "ZAP",
    "Zero-Trust Protocol",
    "Autonomous AI Agents",
    "BFT Consensus",
    "Proof-of-Action",
    "Ed25519",
    "Merkle Mountain Range",
    "Wasmtime Sandboxing",
  ],
  authors: [{ name: "Rivun Protocol Architects" }],
  openGraph: {
    title: "Rivun | Zero-Trust Protocol for AI Agents",
    description:
      "Eliminate prompt injection risks with 64-byte signed frames, 2-phase BFT Proof-of-Action consensus, and air-gapped MMR receipts.",
    url: "https://rivun.dev",
    siteName: "Rivun Protocol",
    type: "website",
  },
  twitter: {
    card: "summary_large_image",
    title: "Rivun | Zero-Trust Protocol for AI Agents",
    description:
      "Eliminate prompt injection risks with 64-byte signed frames, 2-phase BFT Proof-of-Action consensus, and air-gapped MMR receipts.",
  },
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  return (
    <html lang="en" className="dark">
      <body className="bg-[#0A0B0D] text-[#F4F5F7] min-h-screen antialiased selection:bg-[#5B8CFF] selection:text-white">
        {children}
      </body>
    </html>
  );
}
