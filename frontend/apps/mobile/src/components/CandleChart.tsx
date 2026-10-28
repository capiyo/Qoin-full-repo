import React from "react";
import Svg, { Line, Rect } from "react-native-svg";
import type { Candle } from "@qoin/core";

// Dependency-light SVG candlestick chart shared in spirit with apps/web's TradingChart.
export function CandleChart({ candles, width = 340, height = 220 }: { candles: Candle[]; width?: number; height?: number }) {
  const padding = 12;
  const lows = candles.map((c) => c.low);
  const highs = candles.map((c) => c.high);
  const min = Math.min(...lows);
  const max = Math.max(...highs);
  const range = max - min || 1;
  const chartH = height - padding * 2;
  const slotW = (width - padding * 2) / candles.length;
  const bodyW = Math.max(1.5, slotW * 0.6);
  const y = (v: number) => padding + (1 - (v - min) / range) * chartH;

  return (
    <Svg width={width} height={height}>
      {candles.map((c, i) => {
        const x = padding + i * slotW + slotW / 2;
        const up = c.close >= c.open;
        const color = up ? "#22C55E" : "#EF4444";
        const bodyTop = y(Math.max(c.open, c.close));
        const bodyBottom = y(Math.min(c.open, c.close));
        return (
          <React.Fragment key={c.time}>
            <Line x1={x} x2={x} y1={y(c.high)} y2={y(c.low)} stroke={color} strokeWidth={1} />
            <Rect
              x={x - bodyW / 2}
              y={bodyTop}
              width={bodyW}
              height={Math.max(1, bodyBottom - bodyTop)}
              fill={color}
            />
          </React.Fragment>
        );
      })}
    </Svg>
  );
}
