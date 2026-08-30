export const PricingTiers = {
  Community: {
    id: 'community',
    name: 'Community',
    basePriceMonthly: 0,
    includedNodes: 3,
    includedTps: 1_000,
    sla: 'Community Support',
    p99LatencyMs: '<5.0ms',
  },
  Pro: {
    id: 'pro',
    name: 'Pro Cloud',
    basePriceMonthly: 499,
    includedNodes: 25,
    includedTps: 25_000,
    extraNodePrice: 15,
    sla: '99.9% SLA',
    p99LatencyMs: '<1.5ms',
  },
  Enterprise: {
    id: 'enterprise',
    name: 'Enterprise Mesh',
    basePriceMonthly: 2499,
    includedNodes: 100,
    includedTps: 100_000,
    extraNodePrice: 10,
    sla: '99.99% SLA',
    p99LatencyMs: '<0.8ms',
  },
  Sovereign: {
    id: 'sovereign',
    name: 'Sovereign Enclave',
    basePriceMonthly: 9999,
    includedNodes: 500,
    includedTps: 500_000,
    extraNodePrice: 5,
    sla: '99.999% SLA Dedicated',
    p99LatencyMs: '<0.5ms',
  },
};

export function calculatePricing({
  tierId = 'enterprise',
  nodeCount = 20,
  tps = 10_000,
  isAnnual = true,
}) {
  const tierKey = Object.keys(PricingTiers).find(
    (k) => PricingTiers[k].id === tierId.toLowerCase()
  ) || 'Enterprise';
  const tier = PricingTiers[tierKey];

  let monthlyCost = tier.basePriceMonthly;
  if (nodeCount > tier.includedNodes && tier.extraNodePrice) {
    monthlyCost += (nodeCount - tier.includedNodes) * tier.extraNodePrice;
  }

  const effectiveMonthly = isAnnual ? Math.round(monthlyCost * 0.8) : monthlyCost;
  const annualTotal = effectiveMonthly * 12;

  // Traditional cloud baseline cost for comparison
  const cloudCentralizedMonthly = Math.round(nodeCount * 45 + (tps / 1000) * 80);
  const monthlySavings = Math.max(0, cloudCentralizedMonthly - effectiveMonthly);
  const roiPercentage = cloudCentralizedMonthly > 0
    ? Math.round((monthlySavings / cloudCentralizedMonthly) * 100)
    : 0;

  return {
    tier: tier.name,
    tierId: tier.id,
    nodeCount,
    tps,
    isAnnual,
    basePrice: tier.basePriceMonthly,
    monthlyCost: effectiveMonthly,
    annualTotal,
    annualDiscountApplied: isAnnual,
    sla: tier.sla,
    p99Latency: tier.p99LatencyMs,
    cloudBenchmarkMonthly: cloudCentralizedMonthly,
    monthlySavings,
    roiPercentage,
  };
}
