import type { Metadata } from "next";
import { Geist, Geist_Mono } from "next/font/google";
import "./globals.css";
import Navbar from "@/components/layout/Navbar";
import { TooltipProvider } from "@/components/ui/tooltip";

const geistSans = Geist({
  variable: "--font-geist-sans",
  subsets: ["latin"],
});

const geistMono = Geist_Mono({
  variable: "--font-geist-mono",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "ZAP | Universal Protocol for Distributed Systems",
  description: "ZAP is a compact, signed, encrypted, low-latency protocol for moving typed messages between nodes. Built for AI, IoT, Robotics, and Edge environments.",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${geistSans.variable} ${geistMono.variable} h-full antialiased dark`}
    >
      <body className="h-screen overflow-hidden flex flex-col bg-black text-zinc-100 selection:bg-blue-500/30 selection:text-white">
        <TooltipProvider>
          <Navbar />
          <main className="flex-1 mt-16 min-h-0 flex flex-col">
            {children}
          </main>
        </TooltipProvider>
      </body>
    </html>
  );
}
