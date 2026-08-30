import type { Metadata } from "next";
import "./globals.css";
import { Navbar } from "../components/Navbar";
import { Sidebar } from "../components/Sidebar";

export const metadata: Metadata = {
  title: "Rivun Cloud — Enterprise SaaS for ZAP Protocol",
  description: "Multi-tenant zero-trust fleet management, ledger receipts, policy staging, and pack marketplace for Rivun.",
};

export default function RootLayout({
  children,
}: ReadencodedLayoutProps<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className="dark">
      <body className="bg-bg-base text-text-primary min-h-screen flex flex-col antialiased">
        <Navbar />
        <div className="flex-1 flex">
          <Sidebar />
          <main className="flex-1 p-8 overflow-y-auto bg-bg-base max-w-[1600px]">
            {children}
          </main>
        </div>
      </body>
    </html>
  );
}

type ReadencodedLayoutProps<T> = T;
