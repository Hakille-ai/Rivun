import React from "react";
import { Navbar } from "../components/Navbar";
import { HeroSection } from "../components/HeroSection";
import { SwarmVisualizer } from "../components/SwarmVisualizer";
import { ProtocolInnovations } from "../components/ProtocolInnovations";
import { CloudShowcase } from "../components/CloudShowcase";
import { DomainPacksShowcase } from "../components/DomainPacksShowcase";
import { SecurityCompliance } from "../components/SecurityCompliance";
import { PricingCalculator } from "../components/PricingCalculator";
import { ProtocolSandbox } from "../components/ProtocolSandbox";
import { Footer } from "../components/Footer";

export default function HomePage() {
  return (
    <main className="min-h-screen bg-[#0A0B0D] flex flex-col">
      <Navbar />
      <div className="flex-1">
        <HeroSection />
        <SwarmVisualizer />
        <ProtocolInnovations />
        <CloudShowcase />
        <DomainPacksShowcase />
        <SecurityCompliance />
        <PricingCalculator />
        <ProtocolSandbox />
      </div>
      <Footer />
    </main>
  );
}
