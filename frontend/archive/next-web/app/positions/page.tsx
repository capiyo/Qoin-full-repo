"use client";

import { Shell } from "@/components/Shell";
import { PositionsTable } from "@/components/PositionsTable";
import { useMarketFeed, usePositions } from "@/lib/useMarketFeed";

export default function PositionsPage() {
  const { quotes } = useMarketFeed();
  const { data: positions } = usePositions(quotes);

  return (
    <Shell>
      <h1 className="font-heading text-xl font-semibold mb-6">Positions</h1>
      {positions ? (
        <PositionsTable positions={positions} />
      ) : (
        <div className="qoin-card p-6 text-qoin-text-muted text-sm">Loading positions…</div>
      )}
    </Shell>
  );
}
