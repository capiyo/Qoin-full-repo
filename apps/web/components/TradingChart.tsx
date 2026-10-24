"use client";

import { useMemo } from "react";
import type { Candle } from "@qoin/core";

// Lightweight dependency-free SVG candlestick chart. Legible, tabular, and fast —
// swap for lightweight-charts / TradingView if deeper interaction is needed later.
export function TradingChart({ candles, height = 360 }: { candles: Candle[]; height?: number }) {
  const width = 900;
  const padding = 24;

  const { min, max } = useMemo(() => {
    const lows = candles.map((c) => c.low);
    const highs = candles.map((c) => c.high);
    return { min: Math.min(...lows), max: Math.max(...highs) };
  }, [candles]);

  const range = max - min || 1;
  const chartH = height - padding * 2;
  const slotW = (width - padding * 2) / candles.length;
  const bodyW = Math.max(2, slotW * 0.6);

  const y = (v: number) => padding + (1 - (v - min) / range) * chartH;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full h-full"
      role="img"
      aria-label="Price candlestick chart"
    >
      {[0.25, 0.5, 0.75].map((f) => (
        <line
          key={f}
          x1={padding}
          x2={width - padding}
          y1={padding + chartH * f}
          y2={padding + chartH * f}
          stroke="#1E222D"
          strokeWidth={1}
        />
      ))}
      {candles.map((c, i) => {
        const x = padding + i * slotW + slotW / 2;
        const up = c.close >= c.open;
        const color = up ? "#22C55E" : "#EF4444";
        const bodyTop = y(Math.max(c.open, c.close));
        const bodyBottom = y(Math.min(c.open, c.close));
        return (
          <g key={c.time}>
            <line x1={x} x2={x} y1={y(c.high)} y2={y(c.low)} stroke={color} strokeWidth={1} />
            <rect
              x={x - bodyW / 2}
              y={bodyTop}
              width={bodyW}
              height={Math.max(1, bodyBottom - bodyTop)}
              fill={color}
            />
          </g>
        );
      })}
    </svg>
  );
}
