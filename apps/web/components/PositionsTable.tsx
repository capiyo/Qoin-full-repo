import type { Position } from "@qoin/core";

function fmt(n: number, digits = 2) {
  return n.toLocaleString(undefined, { minimumFractionDigits: digits, maximumFractionDigits: digits });
}

export function PositionsTable({ positions }: { positions: Position[] }) {
  return (
    <div className="qoin-card overflow-x-auto">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-left text-qoin-text-muted border-b border-qoin-border">
            <th className="px-4 py-3 font-medium">Symbol</th>
            <th className="px-4 py-3 font-medium">Side</th>
            <th className="px-4 py-3 font-medium text-right">Size</th>
            <th className="px-4 py-3 font-medium text-right">Entry</th>
            <th className="px-4 py-3 font-medium text-right">Mark</th>
            <th className="px-4 py-3 font-medium text-right">Leverage</th>
            <th className="px-4 py-3 font-medium text-right">PnL</th>
          </tr>
        </thead>
        <tbody>
          {positions.map((p) => {
            const positive = p.pnl >= 0;
            return (
              <tr key={p.id} className="border-b border-qoin-border last:border-0">
                <td className="px-4 py-3 font-medium">{p.symbol}</td>
                <td className="px-4 py-3">
                  <span
                    className={`text-xs font-semibold uppercase px-2 py-0.5 rounded ${
                      p.side === "buy"
                        ? "bg-qoin-bullish/15 text-qoin-bullish"
                        : "bg-qoin-bearish/15 text-qoin-bearish"
                    }`}
                  >
                    {p.side}
                  </span>
                </td>
                <td className="px-4 py-3 text-right tabular">{fmt(p.size, 4)}</td>
                <td className="px-4 py-3 text-right tabular">${fmt(p.entryPrice)}</td>
                <td className="px-4 py-3 text-right tabular">${fmt(p.markPrice)}</td>
                <td className="px-4 py-3 text-right tabular">{p.leverage}x</td>
                <td
                  className={`px-4 py-3 text-right tabular font-medium ${
                    positive ? "text-qoin-bullish" : "text-qoin-bearish"
                  }`}
                >
                  {positive ? "+" : ""}
                  {fmt(p.pnl)} ({positive ? "+" : ""}
                  {fmt(p.pnlPct)}%)
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
