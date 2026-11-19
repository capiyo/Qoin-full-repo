import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { User } from "firebase/auth";
import type { FormEvent, ChangeEvent } from "react";
import { signInQoinUser, registerQoinUser, signOutQoinUser, subscribeQoinAuth, uploadQoinAvatar, getQoinArchivedSymbols, saveQoinArchivedSymbols } from "@qoin/core";
import { getInstrumentVector, getInstrumentVectorHistory, getQoinEvents, getQoinSetLedger, getQoinCalendar, getQoinHealth, getQoinInstruments, getQoinNews, getQoinStrategy, getQoinAlgorithmCatalogue, runQoinBacktest, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinAlgorithmCatalogue, type QoinHealth, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent, type QoinSetLedger, type QoinStrategy, type QoinBacktestResult } from "@qoin/core";

type Instrument = {
  symbol: string;
  name: string;
  market: QoinInstrument["asset_class"];
  engineSymbol: string;
  description: string;
  venue: string;
  engineVenue: string;
  provider: string;
  providerStatus?: string;
  archived: boolean;
  color: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", engineSymbol: "BTCUSDT", name: "Bitcoin", market: "Crypto", description: "Bitcoin perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, color: "#f59e0b" },
  { symbol: "ETH/USD", engineSymbol: "ETHUSDT", name: "Ethereum", market: "Crypto", description: "Ether perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, color: "#6477df" },
  { symbol: "SOL/USD", engineSymbol: "SOLUSDT", name: "Solana", market: "Crypto", description: "Solana perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, color: "#8b72e8" },
  { symbol: "NVDA", engineSymbol: "NVDA", name: "NVIDIA", market: "Equities", description: "Semiconductors · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, color: "#76a85a" },
  { symbol: "EUR/USD", engineSymbol: "OANDA:EUR_USD", name: "Euro / US Dollar", market: "FX", description: "Major FX pair", venue: "OANDA", engineVenue: "finnhub", provider: "Finnhub", archived: false, color: "#4d91d8" },
  { symbol: "AAPL", engineSymbol: "AAPL", name: "Apple", market: "Equities", description: "Consumer technology · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, color: "#8895a7" },
];

type Tab = "Archive" | "Explore";
type InstrumentMode = "live" | "backtest";
function instrumentSlug(symbol: string): string { return symbol.replace(/[^a-zA-Z0-9]+/g, "-").replace(/^-|-$/g, "").toUpperCase(); }
function instrumentRoute(item: Instrument, mode: InstrumentMode): string { return `/app/instrument/${encodeURIComponent(instrumentSlug(item.symbol))}/${mode === "backtest" ? "backtest" : "vector"}`; }
function loadCardModes(): Record<string, InstrumentMode> {
  try {
    const raw = window.localStorage.getItem("qoin:archive-card-modes");
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    return Object.fromEntries(Object.entries(parsed).filter((entry): entry is [string, InstrumentMode] => entry[1] === "live" || entry[1] === "backtest"));
  } catch { return {}; }
}

type QoinNotification = { id: string; symbol: string; title: string; body: string; kind: "signal" | "liquidation"; createdAt: number; read: boolean; details: string };

const QOIN_FEATURE_SHORT_NAMES = ["OFI", "FUND", "DEPTH", "VOL", "EVENT", "LIQ"] as const;

type NativeMotionPoint = { id: number; x: number; y: number; vx: number; vy: number };

function VectorMap({ decision, history }: { decision: QoinVectorDecision | null; history: QoinVectorHistory[] }) {
  const bullish = decision?.decision_vector?.[0] ?? 0;
  const bearish = decision?.decision_vector?.[1] ?? 0;
  const points = [...history, ...(decision?.decision_vector ? [{ decision_vector: decision.decision_vector } as QoinVectorHistory] : [])];
  const max = Math.max(0.35, ...points.flatMap((item) => item.decision_vector || [0, 0]));
  const project = (item: QoinVectorHistory) => [10 + (item.decision_vector[0] / max) * 80, 90 - (item.decision_vector[1] / max) * 80] as const;
  const x = 10 + (bullish / max) * 80;
  const y = 90 - (bearish / max) * 80;
  const trajectory = points.map((item) => project(item).join(",")).join(" ");
  const [nativeMotion, setNativeMotion] = useState<NativeMotionPoint[]>([]);
  const nativeMotionRef = useRef<NativeMotionPoint[]>([]);
  const pendingMotionRef = useRef<{ particles: Array<{ id: number; x: number; y: number; vx: number; vy: number; weight: number }>; config: { bullish: number; bearish: number; iterations: number; repulsion: number; damping: number } } | null>(null);
  const motionTimerRef = useRef<ReturnType<typeof window.setTimeout> | null>(null);
  const motionInFlightRef = useRef(false);
  const motionDisposedRef = useRef(false);
  useEffect(() => {
    motionDisposedRef.current = false;
    return () => {
      motionDisposedRef.current = true;
      if (motionTimerRef.current !== null) window.clearTimeout(motionTimerRef.current);
    };
  }, []);
  const scheduleNativeMotion = () => {
    if (motionTimerRef.current !== null || motionInFlightRef.current) return;
    motionTimerRef.current = window.setTimeout(() => {
      motionTimerRef.current = null;
      const request = pendingMotionRef.current;
      pendingMotionRef.current = null;
      if (!request || motionDisposedRef.current) return;
      motionInFlightRef.current = true;
      void import("@tauri-apps/api/core").then(({ invoke }) =>
        invoke<NativeMotionPoint[]>("simulate_vector_motion", request)
      ).then((result) => {
        if (!motionDisposedRef.current) {
          nativeMotionRef.current = result;
          setNativeMotion(result);
        }
      }).catch((error: unknown) => {
        if (!motionDisposedRef.current) console.error("Native vector motion unavailable; retaining SVG fallback.", error);
      }).finally(() => {
        motionInFlightRef.current = false;
        if (pendingMotionRef.current && !motionDisposedRef.current) scheduleNativeMotion();
      });
    }, 80);
  };
  useEffect(() => {
    const runtime = globalThis as typeof globalThis & { __TAURI_INTERNALS__?: unknown };
    if (!runtime.__TAURI_INTERNALS__ || !points.length) {
      pendingMotionRef.current = null;
      nativeMotionRef.current = [];
      setNativeMotion([]);
      return;
    }
    const particles = points.slice(-300).map((item, index) => {
      const previous = nativeMotionRef.current[index];
      return {
        id: index,
        x: previous?.x ?? Math.max(-1, Math.min(1, item.decision_vector[0] / max)),
        y: previous?.y ?? Math.max(-1, Math.min(1, item.decision_vector[1] / max)),
        vx: previous?.vx ?? 0,
        vy: previous?.vy ?? 0,
        weight: 1,
      };
    });
    pendingMotionRef.current = {
      particles,
      config: { bullish: Math.max(-1, Math.min(1, bullish)), bearish: Math.max(-1, Math.min(1, bearish)), iterations: 64, repulsion: 0.012, damping: 0.88 },
    };
    // True trailing throttle: market bursts replace pending inputs but cannot starve the renderer.
    scheduleNativeMotion();
  }, [decision?.decision_vector?.[0], decision?.decision_vector?.[1], history, max, bullish, bearish]);
  return (
    <div className="vector-map">
      <svg viewBox="0 0 100 100" role="img" aria-label="Qoin engine bullish and bearish vector space with native Rust motion on desktop">
        <defs><pattern id="grid" width="10" height="10" patternUnits="userSpaceOnUse"><path d="M 10 0 L 0 0 0 10" fill="none" stroke="#EDF3FF" strokeWidth=".45" /></pattern></defs>
        <rect width="100" height="100" fill="url(#grid)" />
        <path d="M10 8V90H94" fill="none" stroke="#3475E8" strokeWidth=".8" />
        <path d="M10 90L94 90 M10 90L10 8" stroke="#3475E8" strokeWidth=".5" strokeDasharray="2 2" />
        {points.length > 1 && <polyline points={trajectory} fill="none" stroke="#2867DC" strokeWidth="1.1" strokeLinejoin="round" />}
        {nativeMotion.length ? nativeMotion.map((item) => <circle key={item.id} cx={10 + (item.x + 1) * 40} cy={90 - (item.y + 1) * 40} r="1.25" fill="#2867DC" opacity=".78" style={{ transition: "cx 420ms ease, cy 420ms ease" }} />) : history.map((item, index) => { const [px, py] = project(item); return <circle key={item.stored_at || index} cx={px} cy={py} r="1.5" fill="#2867DC" />; })}
        {decision?.decision_vector ? <><line x1="10" y1="90" x2={x} y2={y} stroke="#2867DC" strokeWidth="1.8" /><circle cx={x} cy={y} r="3.8" fill="#2867DC" stroke="white" strokeWidth="1.2" /><circle cx="10" cy="90" r="2" fill="#94a3b8" /></> : <text x="50" y="48" textAnchor="middle" fill="#94a3b8" fontSize="5">NO VECTOR YET</text>}
      </svg>
      <div className="axis-label axis-top">BEARISH COMPONENT ↑</div>
      <div className="axis-label axis-bottom">BULLISH COMPONENT →</div>
    </div>
  );
}

function FeatureSpace({ decision }: { decision: QoinVectorDecision | null }) {
  const labels = ["OFI", "FUND", "DEPTH", "VOL", "EVENT", "LIQ"];
  const values = QOIN_FEATURE_NAMES.map((name) => Math.max(-1, Math.min(1, decision?.features?.[name] ?? 0)));
  const coordinates = values.map((value, index) => {
    const angle = (-Math.PI / 2) + index * (Math.PI * 2 / 6);
    const radius = 18 + value * 14;
    return [50 + Math.cos(angle) * radius, 50 + Math.sin(angle) * radius] as const;
  });
  const polygon = coordinates.map((point) => point.join(",")).join(" ");
  return <section className="feature-space-panel">
    <div className="attention-heading"><strong>Six-dimensional feature space</strong><span>Normalized engine inputs · not a learned embedding</span></div>
    <svg viewBox="0 0 100 100" role="img" aria-label="Six dimensional normalized feature profile">
      {[14, 24, 34].map((radius) => <polygon key={radius} points={labels.map((_, i) => {
        const angle = -Math.PI / 2 + i * (Math.PI * 2 / 6);
        return (50 + Math.cos(angle) * radius) + "," + (50 + Math.sin(angle) * radius);
      }).join(" ")} fill="none" stroke="#e5ebf4" strokeWidth=".5" />)}
      {labels.map((label, i) => {
        const angle = -Math.PI / 2 + i * (Math.PI * 2 / 6);
        return <g key={label}><line x1="50" y1="50" x2={50 + Math.cos(angle) * 35} y2={50 + Math.sin(angle) * 35} stroke="#e5ebf4" strokeWidth=".5" /><text x={50 + Math.cos(angle) * 43} y={50 + Math.sin(angle) * 43} textAnchor="middle" dominantBaseline="middle" fill="#7f8da1" fontSize="4.5">{label}</text></g>;
      })}
      {decision && <><polygon points={polygon} fill="#3475e8" fillOpacity=".16" stroke="#2867DC" strokeWidth="1.2" />{coordinates.map(([x,y],i) => <circle key={labels[i]} cx={x} cy={y} r="1.8" fill="#2867DC" />)}</>}
      {!decision && <text x="50" y="52" textAnchor="middle" fill="#94a3b8" fontSize="5">NO FEATURE VECTOR</text>}
    </svg>
    <div className="feature-space-values">{labels.map((label,index) => <span key={label}>{label} <strong>{values[index].toFixed(3)}</strong></span>)}</div>
  </section>;
}


function DecisionTracePanel({ decision }: { decision: QoinVectorDecision }) {
  const gates = decision.diagnostics;
  const yesNo = (value?: boolean) => value == null ? "Not reported" : value ? "PASS" : "FAIL";
  return <section className="decision-trace-panel">
    <div className="attention-heading"><strong>Decision trace · nothing hidden</strong><span>{decision.strategy_version || "Unversioned"}</span></div>
    <p className="disclaimer">This is the exact calculation record returned by the engine for this decision, including the source event and parameter snapshot.</p>
    {decision.input_event && <div className="trace-source">
      <strong>Triggering source event</strong>
      <span>{decision.input_event.event_type} · {decision.input_event.venue} · {decision.input_event.event_id}</span>
      <small>{new Date(decision.input_event.event_ts_ms).toLocaleString()} · {decision.input_event.state_event_count} accumulated events · state through {decision.input_event.state_last_event_ts_ms ? new Date(decision.input_event.state_last_event_ts_ms).toLocaleString() : "unknown"}</small>
    </div>}
    <div className="trace-table-wrap"><table className="trace-table">
      <thead><tr><th>Feature</th><th>Raw</th><th>Transform</th><th>Scale</th><th>Normalized</th><th>Clamped</th><th>Bull term</th><th>Bear term</th></tr></thead>
      <tbody>{(decision.feature_trace || []).map((item) => <tr key={item.name}>
        <td>{item.name.replaceAll("_", " ")}</td><td>{item.raw_value.toPrecision(5)}</td><td>{item.transform}</td><td>{item.scale}</td>
        <td>{item.normalized_value.toFixed(5)}</td><td>{item.clamped_value.toFixed(5)}</td>
        <td>{item.bullish_weighted_term.toFixed(5)}</td><td>{item.bearish_weighted_term.toFixed(5)}</td>
      </tr>)}</tbody>
    </table></div>
    <div className="trace-gates">
      {gates && [
        ["Spread supplied", yesNo(gates.spread_present)], ["Spread within limit", yesNo(gates.spread_within_limit)],
        ["Event evidence", yesNo(gates.event_intensity_positive)], ["Magnitude threshold", yesNo(gates.magnitude_passed)],
        ["Buy angle", yesNo(gates.buy_angle_passed)], ["Sell angle", yesNo(gates.sell_angle_passed)],
        ["Bullish > bearish", yesNo(gates.bullish_exceeds_bearish)], ["Bearish > bullish", yesNo(gates.bearish_exceeds_bullish)],
      ].map(([label, value]) => <div key={label}><span>{label}</span><strong>{value}</strong></div>)}
    </div>
    <p className="disclaimer">Angle: {decision.angle_degrees?.toFixed(4) ?? "—"}° ({decision.angle_radians?.toFixed(6) ?? "—"} rad) · angular gate: {gates?.angle_limit_degrees ?? "—"}° · magnitude gate: {gates?.magnitude_limit ?? "—"} · spread gate: {gates?.spread_limit_bps ?? "—"} bps. Final action: {decision.action?.toUpperCase() || "WAIT"} — {decision.reason || "No reason supplied"}.</p>
    <details className="trace-details"><summary>Inspect exact raw inputs, parameters and full serialized calculation</summary>
      <h4>Raw inputs (null means the provider did not supply the field)</h4><pre>{JSON.stringify({ ...decision.raw_inputs, liquidation_side: decision.liquidation_side }, null, 2)}</pre>
      <h4>Exact strategy parameters used for this decision</h4><pre>{JSON.stringify(decision.strategy_parameters || null, null, 2)}</pre>
      <h4>Complete decision object</h4><pre>{JSON.stringify(decision, null, 2)}</pre>
    </details>
  </section>;
}

type LedgerRecord = { id: string; timestamp: number | null; kind: "decision" | "observation" | "history"; title: string; summary: string; reference: string };
function ResearchLedger({ decision, history, events }: { decision: QoinVectorDecision | null; history: QoinVectorHistory[]; events: QoinMarketEvent[] }) {
  const records: LedgerRecord[] = [
    ...events.map((event) => ({
      id: "event:" + event.event_id,
      timestamp: event.ts_ms,
      kind: "observation" as const,
      title: event.event_type.replaceAll("_", " "),
      summary: [event.price != null ? "Price " + event.price : null, event.size != null ? "Size " + event.size : null, event.side ? "Side " + event.side : null, event.funding_rate != null ? "Funding " + event.funding_rate : null].filter(Boolean).join(" · ") || "Provider observation recorded",
      reference: event.event_id,
    })),
    ...history.map((item, index) => ({
      id: "history:" + (item.stored_at || index),
      timestamp: item.stored_at ? Date.parse(item.stored_at) : null,
      kind: "history" as const,
      title: "Historical engine decision · " + item.action.toUpperCase(),
      summary: "Magnitude " + item.magnitude.toFixed(4) + " · " + (item.strategy_version || "strategy version unavailable"),
      reference: item.stored_at || "Historical vector record",
    })),
    ...(decision ? [{
      id: "decision:" + (decision.input_event?.event_id || decision.strategy_version || decision.symbol),
      timestamp: decision.input_event?.event_ts_ms ?? null,
      kind: "decision" as const,
      title: "Latest engine evaluation · " + (decision.action || "wait").toUpperCase(),
      summary: decision.reason || "No rationale was supplied by the engine.",
      reference: decision.input_event?.event_id || "Source event reference unavailable",
    }] : []),
  ].sort((left, right) => (right.timestamp ?? -1) - (left.timestamp ?? -1)).slice(0, 40);
  const formatTime = (value: number | null) => value != null && Number.isFinite(value) ? new Date(value).toLocaleString() : "Timestamp unavailable";
  return <section aria-label="Research decision and evidence ledger" style={{ display: "grid", gap: 12, padding: 16, border: "1px solid #e4eaf3", borderRadius: 16, background: "#fff" }}>
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap" }}>
      <div><strong style={{ color: "#243750", fontSize: 14 }}>Decision &amp; evidence ledger</strong><p style={{ margin: "4px 0 0", color: "#8794a7", fontSize: 10 }}>A readable history of engine evaluations and their recorded market evidence.</p></div>
      <span style={{ color: "#2867DC", fontSize: 10, fontWeight: 800 }}>{records.length} RECORDS</span>
    </div>
    {records.length ? <div style={{ display: "grid", gap: 8 }}>
      {records.map((record) => <details key={record.id} style={{ padding: "11px 12px", border: "1px solid #edf1f6", borderRadius: 11, background: record.kind === "decision" ? "#f5f8ff" : "#fff" }}>
        <summary style={{ display: "flex", alignItems: "center", gap: 9, cursor: "pointer", listStyle: "none", flexWrap: "wrap" }}>
          <span style={{ minWidth: 76, color: record.kind === "decision" ? "#2867DC" : "#8190a4", fontSize: 9, fontWeight: 800, textTransform: "uppercase" }}>{record.kind}</span>
          <strong style={{ flex: "1 1 180px", color: "#34465f", fontSize: 11, textTransform: "capitalize" }}>{record.title}</strong>
          <small style={{ color: "#9aa6b5", fontSize: 9 }}>{formatTime(record.timestamp)}</small>
        </summary>
        <p style={{ margin: "9px 0 5px", color: "#52637a", fontSize: 10, lineHeight: 1.5 }}>{record.summary}</p>
        <small style={{ color: "#8996a8", fontSize: 9 }}>Source reference · {record.reference}</small>
      </details>)}
    </div> : <div style={{ padding: 16, borderRadius: 11, background: "#f7f9fc", color: "#8794a7", fontSize: 11 }}>No persisted evidence or decision records are available for this instrument yet.</div>}
    <p style={{ margin: 0, color: "#9aa6b5", fontSize: 9, lineHeight: 1.5 }}>Read-only research history. These entries reflect records currently returned by the API; this view does not claim that set relationships or ledger-grade immutable audit storage are implemented.</p>
  </section>;
}


function VectorSpaceMechanics({ decision }: { decision: QoinVectorDecision | null }) {
  const names = QOIN_FEATURE_NAMES;
  const raw = names.map((name) => decision?.features?.[name] ?? 0);
  const attended = names.map((_, index) => decision?.attended_features?.[index] ?? 0);
  const attention = decision?.attention ?? [];
  const directions = names.map((_, index) => -Math.PI / 2 + index * (Math.PI * 2 / Math.max(names.length, 1)));
  const endpoints = attended.map((value, index) => ({
    x: 50 + Math.cos(directions[index]) * Math.max(-1, Math.min(1, value)) * 34,
    y: 50 - Math.sin(directions[index]) * Math.max(-1, Math.min(1, value)) * 34,
  }));
  const links = names.flatMap((_, from) => names.map((__, to) => ({ from, to, weight: attention[from]?.[to] ?? 0 })).filter((link) => link.from !== link.to && link.weight >= 0.18))
    .sort((a, b) => b.weight - a.weight).slice(0, 12);
  const output = decision?.decision_vector ?? [0, 0];
  return <section className="vector-space-mechanics" aria-label="Six-dimensional vector space and output">
    <div className="attention-heading"><strong>Vector space · six engine dimensions</strong><span>2D projection of the actual R⁶ feature state</span></div>
    <p className="disclaimer">Each radial basis direction represents one normalized engine feature. Arrow length and sign use its attended value. The plot is a fixed 2D projection for inspection; calculations remain in six dimensions. The bullish/bearish decision vector lives in a separate output space below.</p>
    <div className="vector-space-layout">
      <div className="vector-space-plot">
        <svg viewBox="0 0 100 100" role="img" aria-label="Six-dimensional feature vectors, directed attention links and resultant decision vector">
          <defs>
            <pattern id="qoin-vector-grid" width="10" height="10" patternUnits="userSpaceOnUse"><path d="M10 0H0V10" fill="none" stroke="#263b33" strokeWidth=".35"/></pattern>
            <marker id="qoin-attention-arrow" markerWidth="5" markerHeight="5" refX="4" refY="2.5" orient="auto"><path d="M0 0L5 2.5L0 5Z" fill="#D6A85C"/></marker>
          </defs>
          <rect width="100" height="100" fill="url(#qoin-vector-grid)"/>
          {[12,22,32].map((radius)=><circle key={radius} cx="50" cy="50" r={radius} fill="none" stroke="#365247" strokeWidth=".35" strokeDasharray="1.5 2"/>)}
          <path d="M5 50H95 M50 5V95" stroke="#668477" strokeWidth=".5"/>
          {links.map((link,index)=><line key={index} x1={endpoints[link.from].x} y1={endpoints[link.from].y} x2={endpoints[link.to].x} y2={endpoints[link.to].y} stroke="#D6A85C" strokeWidth={0.35+link.weight*1.1} opacity={0.18+link.weight*.55} markerEnd="url(#qoin-attention-arrow)"/>)}
          {endpoints.map((point,index)=><g key={names[index]}>
            <line x1="50" y1="50" x2={point.x} y2={point.y} stroke={["#64A7E8","#64C69B","#D9A65D","#B69AE8","#DB7E85","#72C6CF"][index%6]} strokeWidth="1.15"/>
            <circle cx={point.x} cy={point.y} r="1.8" fill={["#64A7E8","#64C69B","#D9A65D","#B69AE8","#DB7E85","#72C6CF"][index%6]}/>
            <text x={50+Math.cos(directions[index])*43} y={50-Math.sin(directions[index])*43} textAnchor="middle" dominantBaseline="middle" fill="#c7d6ce" fontSize="4.1">{names[index].replaceAll("_"," ").slice(0,10)}</text>
          </g>)}
          <circle cx="50" cy="50" r="1.8" fill="#e6f0e9"/>
        </svg>
        <div className="vector-space-legend"><span><i/>Feature vectors</span><span><i/>Directed attention links</span></div>
      </div>
      <div className="vector-space-readout">
        <div className="attention-heading"><strong>Coordinates</strong><span>Raw → attended</span></div>
        {names.map((name,index)=><div className="vector-coordinate-row" key={name}><span>{name.replaceAll("_"," ")}</span><strong>{raw[index].toFixed(4)} → {attended[index].toFixed(4)}</strong></div>)}
        <div className="vector-interaction-summary"><strong>Top directed attention links</strong>
          {links.slice(0,5).map((link,index)=><div key={index}><span>{names[link.from]} → {names[link.to]}</span><b>{link.weight.toFixed(4)}</b></div>)}
          {!links.length && <small>No non-diagonal attention links above 0.18 were returned.</small>}
        </div>
      </div>
    </div>
    <section className="decision-output-visual" aria-label="Resultant decision vector plotted in bullish and bearish output coordinates">
      <div className="attention-heading"><strong>Resultant vector · decision space</strong><span>Actual persisted backend coordinates</span></div>
      <div className="decision-output-layout">
        <svg viewBox="0 0 100 100" role="img" aria-label={`Decision vector bullish ${output[0].toFixed(4)}, bearish ${output[1].toFixed(4)}`}>
          <defs><marker id="qoin-resultant-arrow" markerWidth="5" markerHeight="5" refX="4" refY="2.5" orient="auto"><path d="M0 0L5 2.5L0 5Z" fill="#2867DC"/></marker></defs>
          {[15,30].map((radius)=><circle key={radius} cx="50" cy="50" r={radius} fill="none" stroke="#DCE5F0" strokeDasharray="2 2" strokeWidth=".7"/>)}
          <path d="M5 50H95 M50 5V95" stroke="#9AAAC0" strokeWidth=".8"/>
          <path d="M50 50L50 7 M50 50L93 50" stroke="#B7C5D7" strokeWidth=".6"/>
          <line x1="50" y1="50" x2={50 + Math.max(-1,Math.min(1,output[0]))*38} y2={50 - Math.max(-1,Math.min(1,output[1]))*38} stroke="#2867DC" strokeWidth="2.1" markerEnd="url(#qoin-resultant-arrow)"/>
          <circle cx={50 + Math.max(-1,Math.min(1,output[0]))*38} cy={50 - Math.max(-1,Math.min(1,output[1]))*38} r="2.5" fill="#2867DC" stroke="#FFFFFF" strokeWidth="1"/>
          <circle cx="50" cy="50" r="1.7" fill="#53677F"/>
          <text x="95" y="56" textAnchor="end" fill="#2867DC" fontSize="4">Bullish +</text>
          <text x="52" y="7" fill="#159B78" fontSize="4">Bearish +</text>
          <text x="5" y="56" fill="#8794A7" fontSize="4">Bullish −</text>
          <text x="52" y="96" fill="#8794A7" fontSize="4">Bearish −</text>
        </svg>
        <div className="decision-output-readout">
          <div><span>Bullish coordinate</span><strong>{output[0].toFixed(6)}</strong></div>
          <div><span>Bearish coordinate</span><strong>{output[1].toFixed(6)}</strong></div>
          <div><span>Vector magnitude</span><strong>{Math.hypot(output[0],output[1]).toFixed(6)}</strong></div>
          <div><span>Decision</span><strong className={decision?.action === "BUY" ? "decision-buy" : decision?.action === "SELL" ? "decision-sell" : ""}>{decision?.action?.toUpperCase() || "WAIT"}</strong></div>
        </div>
      </div>
      <p className="disclaimer">The arrow is the stored two-coordinate decision vector, not a second signal calculation. The chart clips its display to the unit frame; the exact coordinates remain printed beside it.</p>
    </section>
    <div className="vector-output-bridge">
      <div><span>Resultant decision vector · backend output</span><strong>({output[0].toFixed(5)}, {output[1].toFixed(5)})</strong></div>
      <span className="vector-output-arrow">→</span>
      <div><span>Decision function result</span><strong>{decision?.action?.toUpperCase() || "WAIT"} · {decision?.reason || "No decision rationale available"}</strong></div>
    </div>
    <p className="disclaimer">Attention links are deterministic engine weights, not causal proof. The resultant vector and action are read directly from the persisted decision; the interface does not recalculate or replace them.</p>
  </section>;
}
function SetLedgerPanel({ ledger, status }: { ledger: QoinSetLedger | null; status: string }) {
  const firstSet = ledger?.sets[0];
  const secondSet = ledger?.sets[1];
  const firstMembers = new Set(firstSet?.member_event_ids ?? []);
  const secondMembers = new Set(secondSet?.member_event_ids ?? []);
  const intersectionCount = [...firstMembers].filter((id) => secondMembers.has(id)).length;
  const unionCount = new Set([...firstMembers, ...secondMembers]).size;
  const overlapMatrix = (ledger?.sets ?? []).map((set) => (ledger?.sets ?? []).map((other) => {
    const otherMembers = new Set(other.member_event_ids);
    const count = set.member_event_ids.filter((id) => otherMembers.has(id)).length;
    const denominator = Math.min(set.measure.counting_measure, other.measure.counting_measure);
    return { count, intensity: denominator > 0 ? count / denominator : 0 };
  }));
  return <section aria-label="Version 2 set and relation ledger" style={{ display: "grid", gap: 12, padding: 16, border: "1px solid #E8EDF4", borderRadius: 16, background: "#FFFFFF", color: "#243750" }}>
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", gap: 10, flexWrap: "wrap" }}>
      <div><strong style={{ fontSize: 14 }}>Version 2 · Set &amp; relation ledger</strong><p style={{ margin: "4px 0 0", color: "#8794A7", fontSize: 10 }}>Finite sets derived from persisted source events; separate from V1 signal calculations.</p></div>
      <span style={{ color: "#2867DC", fontSize: 9, fontWeight: 800 }}>RESEARCH · READ ONLY</span>
    </div>
    <p aria-live="polite" style={{ margin: 0, color: "#8794A7", fontSize: 9 }}>{status}</p>
    {!ledger ? <p style={{ margin: 0, color: "#8794A7", fontSize: 11 }}>{status}</p> : <>
      <div style={{ display: "flex", gap: 14, flexWrap: "wrap", color: "#8794A7", fontSize: 10 }}>
        <span>Universe <strong style={{ color: "#243750" }}>{ledger.universe_count}</strong></span>
        {ledger.source_event_count != null ? <span>Source events <strong style={{ color: "#243750" }}>{ledger.source_event_count}</strong></span> : null}
        {ledger.durable_observation_count != null ? <span>Durable observations <strong style={{ color: "#243750" }}>{ledger.durable_observation_count}</strong></span> : null}
        {ledger.durable_relation_count != null ? <span>Stored relation records <strong style={{ color: "#243750" }}>{ledger.durable_relation_count}</strong></span> : null}
        {ledger.durable_history_complete != null ? <span>History <strong style={{ color: ledger.durable_history_complete ? "#16845B" : "#B7791F" }}>{ledger.durable_history_complete ? "complete" : "incomplete"}</strong></span> : null}
        {ledger.relation_coverage_complete != null ? <span>Relation coverage <strong style={{ color: ledger.relation_coverage_complete ? "#16845B" : "#B7791F" }}>{ledger.relation_coverage_complete ? "complete" : "bounded"}</strong></span> : null}
        <span>Sets <strong style={{ color: "#243750" }}>{ledger.sets.length}</strong></span>
        <span>Relations <strong style={{ color: "#243750" }}>{ledger.relations.length}</strong></span>
        <span>Invalid skipped <strong style={{ color: "#243750" }}>{ledger.skipped_invalid_events}</strong></span>
        <span>Limit <strong style={{ color: "#243750" }}>{ledger.requested_limit}</strong></span>
      </div>
      {ledger.universe_truncated ? <p style={{ margin: 0, color: "#D99528", fontSize: 9 }}>Universe capped at {ledger.requested_limit} source documents. Measures and relations below apply only to this returned sample.</p> : null}
      {firstSet && secondSet ? <div className="set-relationship-visual" aria-label="Observed membership overlap between the first two backend evidence sets">
        <div className="set-venn-wrap">
          <svg viewBox="0 0 320 150" role="img" aria-label={firstSet.label + " and " + secondSet.label + " observed set intersection"}>
            <circle cx="119" cy="72" r="54" fill="#3985D6" fillOpacity=".24" stroke="#3985D6" strokeWidth="1.5"/>
            <circle cx="201" cy="72" r="54" fill="#159B78" fillOpacity=".24" stroke="#159B78" strokeWidth="1.5"/>
            <text x="88" y="17" textAnchor="middle" fill="#3985D6" fontSize="9" fontWeight="700">{firstSet.label.slice(0,20)}</text>
            <text x="232" y="17" textAnchor="middle" fill="#159B78" fontSize="9" fontWeight="700">{secondSet.label.slice(0,20)}</text>
            <text x="91" y="76" textAnchor="middle" fill="currentColor" fontSize="14" fontWeight="700">{Math.max(0, firstSet.measure.counting_measure - intersectionCount)}</text>
            <text x="160" y="76" textAnchor="middle" fill="currentColor" fontSize="14" fontWeight="700">{intersectionCount}</text>
            <text x="229" y="76" textAnchor="middle" fill="currentColor" fontSize="14" fontWeight="700">{Math.max(0, secondSet.measure.counting_measure - intersectionCount)}</text>
            <text x="160" y="139" textAnchor="middle" fill="#8794A7" fontSize="8">Exclusive · shared · exclusive</text>
          </svg>
          <div className="set-union-total"><span>Union cardinality</span><strong>{unionCount}</strong></div>
        </div>
        <p className="set-venn-note">Observed event-ID membership overlap. Circle areas are illustrative and are not scaled to set size.</p>
      </div> : null}
      <section className="set-cardinality-map" aria-label="Observed cardinality of all backend sets">
        <div className="attention-heading"><strong>Set measure map</strong><span>Counting measure · |A|</span></div>
        <div className="set-cardinality-axis"><span>0</span><span>{ledger.universe_count} events in universe</span></div>
        {ledger.sets.map((item) => <div className="set-cardinality-row" key={item.set_id}>
          <span title={item.label}>{item.label}</span>
          <div className="set-cardinality-track"><i style={{width: ledger.universe_count ? (item.measure.counting_measure / ledger.universe_count * 100) + "%" : "0%"}} /></div>
          <strong>{item.measure.counting_measure}</strong>
        </div>)}
        <p className="set-venn-note">Bars are exact cardinalities from returned event IDs. A zero-length bar means the set has no observed members in this universe.</p>
      </section>
      <section className="set-overlap-map" aria-label="Pairwise set intersection matrix">
        <div className="attention-heading"><strong>Set intersection matrix</strong><span>Each cell shows |A ∩ B|</span></div>
        <div className="set-overlap-scroll">
          <div className="set-overlap-grid" style={{gridTemplateColumns: `104px repeat(${ledger.sets.length}, minmax(24px, 1fr))`}}>
            <span className="set-overlap-corner">A ∩ B</span>
            {ledger.sets.map((item) => <span className="set-overlap-heading" key={"col-"+item.set_id} title={item.label}>{item.set_id.split("_").map((part)=>part[0]).join("").toUpperCase()}</span>)}
            {ledger.sets.map((item,rowIndex) => <Fragment key={"row-"+item.set_id}>
              <span className="set-overlap-row-label" title={item.label}>{item.label}</span>
              {ledger.sets.map((other,columnIndex) => {
                const cell=overlapMatrix[rowIndex][columnIndex];
                return <span key={other.set_id} className="set-overlap-cell" title={item.label+" ∩ "+other.label+" = "+cell.count} style={{backgroundColor: `rgba(40,103,220,${0.06+cell.intensity*0.42})`}}>{cell.count}</span>;
              })}
            </Fragment>)}
          </div>
        </div>
        <p className="set-venn-note">Intersection counts are computed from shared event IDs, not inferred from the visual layout. Stronger cells mean a larger share of the smaller set is shared.</p>
      </section>
      <section className="relation-flow-map" aria-label="Observed temporal relation graph">
        <div className="attention-heading"><strong>Relation flow</strong><span>Source set → target set · observed edges</span></div>
        {ledger.relations.map((item) => <div className="relation-flow-row" key={item.relation_id}>
          <span>{item.source_set.replaceAll("_"," ")}</span>
          <svg viewBox="0 0 100 24" role="img" aria-label={item.edge_count + " observed temporal precedence edges"}>
            <defs><marker id={"relation-arrow-" + item.relation_id} markerWidth="5" markerHeight="5" refX="4" refY="2.5" orient="auto"><path d="M0 0L5 2.5L0 5Z" fill="#159B78"/></marker></defs>
            <circle cx="7" cy="12" r="4" fill="#3985D6"/><line x1="12" y1="12" x2="87" y2="12" stroke="#159B78" strokeWidth={item.edge_count ? "1.5" : ".7"} strokeDasharray={item.edge_count ? "none" : "2 2"} markerEnd={"url(#relation-arrow-" + item.relation_id + ")"} opacity={item.edge_count ? "1" : ".45"}/><circle cx="93" cy="12" r="4" fill="#159B78"/>
            <text x="50" y="8" textAnchor="middle" fill="#52637A" fontSize="5">{item.edge_count} edges</text>
          </svg>
          <span>{item.target_set.replaceAll("_"," ")}</span>
        </div>)}
        <p className="set-venn-note">An arrow means at least one ordered pair exists within the configured time window. It does not imply causation.</p>
      </section>
      <div style={{ display: "grid", gap: 7 }}>
        <strong style={{ fontSize: 11 }}>Set membership &amp; measure</strong>
        {ledger.sets.map((item) => <details key={item.set_id} style={{ padding: "9px 10px", borderRadius: 10, background: "#FBFCFE", border: "1px solid #E8EDF4" }}>
          <summary style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, cursor: "pointer", fontSize: 10 }}>
            <span>{item.label}</span><strong style={{ color: "#2867DC" }}>{item.measure.counting_measure} members</strong>
          </summary>
          <p style={{ margin: "7px 0 4px", color: "#8794A7", fontSize: 9 }}>Counting measure · {item.measure.counting_measure} events · trade-size measure {item.measure.trade_size_measure.toFixed(6)} {item.measure.trade_size_unit} · observed {item.measure.trade_size_observed_count} / missing {item.measure.trade_size_missing_count}</p>
          <p style={{ margin: 0, color: "#8996A8", fontSize: 8, overflowWrap: "anywhere" }}>Members: {item.member_event_ids.slice(0, 8).join(", ") || "∅"}{item.member_event_ids.length > 8 ? " … +" + (item.member_event_ids.length - 8) + " more" : ""}</p>
        </details>)}
      </div>
      <div style={{ display: "grid", gap: 7 }}>
        <strong style={{ fontSize: 11 }}>Temporal relations</strong>
        {ledger.relations.map((item) => <details key={item.relation_id} style={{ padding: "9px 10px", borderRadius: 10, background: "#FBFCFE", border: "1px solid #E8EDF4" }}>
          <summary style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 8, cursor: "pointer", fontSize: 10 }}>
            <span>{item.label}</span><strong style={{ color: "#2867DC" }}>{item.edge_count} edges</strong>
          </summary>
          <p style={{ margin: "7px 0 4px", color: "#8794A7", fontSize: 9 }}>Window {item.window_ms} ms · mean lag {item.mean_lag_ms == null ? "—" : item.mean_lag_ms.toFixed(1) + " ms"}</p>
          <p style={{ margin: "0 0 6px", color: "#8996A8", fontSize: 8 }}>{item.interpretation}</p>
          {item.edges.slice(0, 8).map((edge, index) => <p key={item.relation_id + index} style={{ margin: "3px 0", color: "#52637A", fontSize: 8, overflowWrap: "anywhere" }}>{edge.from_event_id} → {edge.to_event_id} · {edge.lag_ms} ms</p>)}
          {item.edge_count > 8 ? <small style={{ color: "#8996A8" }}>Showing 8 of {item.edge_count} relation edges{item.edges_truncated ? " (sample capped at 100)" : ""}</small> : null}
        </details>)}
      </div>
      <div style={{ display: "grid", gap: 4 }}>
        {ledger.limitations.map((item) => <p key={item} style={{ margin: 0, color: "#8996A8", fontSize: 8, lineHeight: 1.45 }}>• {item}</p>)}
      </div>
      <small style={{ color: "#8996A8", fontSize: 8 }}>Schema {ledger.schema_version} · {ledger.persistence}</small>
    </>}
  </section>;
}

function BacktestPanel({ symbol, venue, authenticated, onRequireAuth }: { symbol: string; venue: string; authenticated: boolean; onRequireAuth: () => void }) {
  const [capital,setCapital]=useState("10000");
  const [fee,setFee]=useState("10");
  const [slippage,setSlippage]=useState("5");
  const [latency,setLatency]=useState("0");
  const [participation,setParticipation]=useState("0.1");
  const [impact,setImpact]=useState("5");
  const [from,setFrom]=useState("");
  const [to,setTo]=useState("");
  const [result,setResult]=useState<QoinBacktestResult|null>(null);
  const [status,setStatus]=useState("");
  const [busy,setBusy]=useState(false);
  const [pendingRun,setPendingRun]=useState(false);
  const run=async()=>{
    if (!authenticated) { setPendingRun(true); onRequireAuth(); setStatus("Sign in to continue the historical research replay."); return; }
    setBusy(true);setStatus("");setResult(null);
    try {
      const fromMs=from?Date.parse(from):undefined; const toMs=to?Date.parse(to):undefined;
      const initialCapital=Number(capital); const feeBps=Number(fee); const slippageBps=Number(slippage); const latencyMs=Number(latency); const participationRate=Number(participation); const impactBps=Number(impact);
      if(from && !Number.isFinite(fromMs)) throw new Error("Invalid start date");
      if(to && !Number.isFinite(toMs)) throw new Error("Invalid end date");
      if(fromMs !== undefined && toMs !== undefined && fromMs > toMs) throw new Error("Start date must be before or equal to end date.");
      if(!Number.isFinite(initialCapital) || initialCapital <= 0) throw new Error("Starting capital must be a finite amount greater than zero.");
      if(!Number.isFinite(feeBps) || feeBps < 0) throw new Error("Fee must be a finite, non-negative number of basis points.");
      if(!Number.isFinite(slippageBps) || slippageBps < 0) throw new Error("Slippage must be a finite, non-negative number of basis points.");
      if(!Number.isInteger(latencyMs) || latencyMs < 0 || latencyMs > 60000) throw new Error("Latency must be a whole number between 0 and 60000 milliseconds.");
      if(!Number.isFinite(participationRate) || participationRate <= 0 || participationRate > 1) throw new Error("Maximum liquidity participation must be greater than 0 and no more than 1.");
      if(!Number.isFinite(impactBps) || impactBps < 0 || impactBps > 1000 || slippageBps + impactBps >= 10000) throw new Error("Market impact must be 0–1000 bps and combined slippage/impact must stay below 10000 bps.");
      const output=await runQoinBacktest({symbol,venue,from_ts_ms:fromMs,to_ts_ms:toMs,
        initial_capital:initialCapital,fee_bps:feeBps,slippage_bps:slippageBps,latency_ms:latencyMs,max_participation_rate:participationRate,market_impact_bps:impactBps,limit:10000});
      setResult(output);setStatus("Replay completed from persisted events.");
    } catch(error) {setStatus(error instanceof Error?error.message:"Backtest failed");}
    finally {setBusy(false);}
  };
  useEffect(() => {
    if (authenticated && pendingRun && !busy) { setPendingRun(false); void run(); }
  }, [authenticated, pendingRun, busy]);
  return <section className="backtest-panel">
    <div className="attention-heading"><strong>Historical event replay</strong><span>Paper simulation · no orders</span></div>
    <p className="disclaimer">Replays only raw observations already stored in Qoin. It does not download missing history. Empty dates use the full stored range for this instrument.</p>
    <div className="backtest-controls">
      <label>From <input type="datetime-local" value={from} onChange={e=>setFrom(e.target.value)} /></label>
      <label>To <input type="datetime-local" value={to} onChange={e=>setTo(e.target.value)} /></label>
      <label>Starting capital <input type="number" min="0.01" step="any" value={capital} onChange={e=>setCapital(e.target.value)} /></label>
      <label>Fee (bps) <input type="number" min="0" step="any" value={fee} onChange={e=>setFee(e.target.value)} /></label>
      <label>Slippage (bps) <input type="number" min="0" step="any" value={slippage} onChange={e=>setSlippage(e.target.value)} /></label>
      <label>Latency (ms) <input type="number" min="0" max="60000" step="1" value={latency} onChange={e=>setLatency(e.target.value)} /></label>
      <label>Max liquidity participation (0–1) <input type="number" min="0.001" max="1" step="0.01" value={participation} onChange={e=>setParticipation(e.target.value)} /></label>
      <label>Market impact (bps) <input type="number" min="0" max="1000" step="any" value={impact} onChange={e=>setImpact(e.target.value)} /></label>
      <button onClick={run} disabled={busy}>{busy?"Replaying…":"Run replay"}</button>
    </div>
    {status && <p className="disclaimer" role="status">{status}</p>}
    {result && <>
      <div className="backtest-metrics">
        <div><span>Net P/L</span><strong>{result.net_pnl.toFixed(2)}</strong></div>
        <div><span>Net return</span><strong>{result.net_return_pct.toFixed(3)}%</strong></div>
        <div><span>Max drawdown</span><strong>{result.max_drawdown_pct.toFixed(3)}%</strong></div>
        <div><span>Final equity</span><strong>{result.final_equity.toFixed(2)}</strong></div>
        <div><span>Events / decisions</span><strong>{result.events_replayed} / {result.decisions}</strong></div>
        <div><span>Largest raw-event gap</span><strong>{(result.max_event_gap_ms / 1000).toFixed(1)}s</strong></div>
        <div><span>Gaps over 5 minutes</span><strong>{result.event_gaps_over_5m}</strong></div>
        <div><span>BUY / SELL / WAIT</span><strong>{result.buy_decisions} / {result.sell_decisions} / {result.wait_decisions}</strong></div>
        <div><span>Fees paid</span><strong>{result.fees_paid.toFixed(4)}</strong></div>
        <div><span>Slippage cost</span><strong>{result.slippage_paid.toFixed(4)}</strong></div>
        <div><span>Submission latency</span><strong>{result.latency_ms} ms</strong></div>
        <div><span>Max liquidity participation</span><strong>{(result.max_participation_rate * 100).toFixed(1)}%</strong></div>
        <div><span>Impact coefficient</span><strong>{result.market_impact_bps.toFixed(2)} bps</strong></div>
        <div><span>Market impact cost</span><strong>{result.market_impact_paid.toFixed(4)}</strong></div>
        <div><span>Net funding (received +)</span><strong>{result.net_funding_cashflow.toFixed(4)}</strong></div>
        <div><span>Funding settlements</span><strong>{result.funding_settlements}</strong></div>
      </div>
      <p className="disclaimer">Strategy {result.strategy_key}@{result.strategy_version} · {result.venue} · {new Date(result.from_ts_ms).toLocaleString()} → {new Date(result.to_ts_ms).toLocaleString()}</p>
      <h4>Execution cost stress (cost-only; fills not recomputed)</h4>
      <div className="trace-table-wrap"><table className="trace-table"><thead><tr><th>Scenario</th><th>Cost multiplier</th><th>Fees</th><th>Slippage</th><th>Market impact</th><th>Estimated net P/L</th><th>Estimated return</th></tr></thead>
      <tbody>{result.execution_stress_scenarios.map(scenario=><tr key={scenario.name}><td>{scenario.name}</td><td>{scenario.cost_multiplier.toFixed(1)}×</td><td>{scenario.fees_paid.toFixed(4)}</td><td>{scenario.slippage_paid.toFixed(4)}</td><td>{scenario.market_impact_paid.toFixed(4)}</td><td>{scenario.estimated_net_pnl.toFixed(4)}</td><td>{scenario.estimated_return_pct.toFixed(3)}%</td></tr>)}</tbody></table></div>
      <div className="trace-table-wrap"><table className="trace-table"><thead><tr><th>Order</th><th>Decision / submission / fill</th><th>Action</th><th>Price</th><th>This fill / cumulative / target / remaining</th><th>Fee</th><th>Slippage</th><th>Impact (bps / cost)</th><th>Realized P/L</th><th>Equity</th><th>Reason</th></tr></thead>
      <tbody>{result.fills.map((fill,index)=><tr key={fill.order_id+"-"+fill.event_id+"-"+index}><td>{fill.order_id} · {fill.status}</td><td>{new Date(fill.decision_ts_ms).toLocaleTimeString()} / {new Date(fill.submitted_ts_ms).toLocaleTimeString()} / {new Date(fill.ts_ms).toLocaleTimeString()}</td><td>{fill.action}</td><td>{fill.price.toPrecision(8)}</td><td>{fill.quantity.toPrecision(6)} / {fill.filled_quantity.toPrecision(6)} / {fill.target_quantity.toPrecision(6)} / {fill.remaining_quantity.toPrecision(6)}</td><td>{fill.fee.toFixed(4)}</td><td>{fill.slippage_cost.toFixed(4)}</td><td>{fill.market_impact_bps.toFixed(3)} / {fill.market_impact_cost.toFixed(4)}</td><td>{fill.realized_pnl.toFixed(4)}</td><td>{fill.equity_after.toFixed(4)}</td><td>{fill.reason}</td></tr>)}</tbody></table></div>
      {result.unfilled_pending_order && <p className="disclaimer">Unfilled {result.unfilled_pending_order.status} {result.unfilled_pending_order.action.toUpperCase()} order {result.unfilled_pending_order.order_id} · filled {result.unfilled_pending_order.filled_quantity.toPrecision(6)} / {result.unfilled_pending_order.target_quantity?.toPrecision(6) ?? "not sized"} · remaining {result.unfilled_pending_order.remaining_quantity?.toPrecision(6) ?? "not sized"} · submitted {new Date(result.unfilled_pending_order.submitted_ts_ms).toLocaleString()}.</p>}
      <details className="trace-details"><summary>Replay assumptions and full result</summary><ul>{result.assumptions.map((item,index)=><li key={index}>{item}</li>)}</ul><pre>{JSON.stringify(result,null,2)}</pre></details>
    </>}
  </section>;
}

function InstrumentCard({ item, onOpen, action, live, mode, onToggleMode }: { item: Instrument; onOpen: () => void; action?: { label: string; onClick: () => void }; mode: "live" | "backtest"; onToggleMode: () => void; live?: { state?: QoinMarketState; decision?: QoinVectorDecision; event?: QoinStreamUpdate["event"] } }) {
  const [page,setPage]=useState<0|1>(0);
  const [ledger,setLedger]=useState<QoinSetLedger|null>(null);
  const [ledgerStatus,setLedgerStatus]=useState<"idle"|"loading"|"loaded"|"unavailable">("idle");
  const touchStart=useRef<number|null>(null);
  const decision=live?.decision;
  const price=live?.state?.last_price;
  const sourceTime=decision?.input_event?.event_ts_ms;
  useEffect(()=>{
    if(page!==1||ledger||ledgerStatus==="loading"||ledgerStatus==="unavailable")return;
    let active=true;
    setLedgerStatus("loading");
    getQoinSetLedger(item.engineSymbol,1000).then((result)=>{if(active){setLedger(result);setLedgerStatus("loaded");}}).catch(()=>{if(active)setLedgerStatus("unavailable");});
  },[page,item.engineSymbol,ledger,ledgerStatus]);
  return <article className="instrument-card qoin-instrument-card"
    onTouchStart={(event)=>{touchStart.current=event.touches[0]?.clientX??null;}}
    onTouchEnd={(event)=>{const end=event.changedTouches[0]?.clientX;if(touchStart.current!=null&&end!=null&&Math.abs(end-touchStart.current)>38)setPage(end<touchStart.current?1:0);touchStart.current=null;}}>
    <button className="card-main" onClick={onOpen} aria-label={`Open ${item.name} research`}>
      <span className="asset-mark" style={{background:item.color}}>{item.symbol.slice(0,1)}</span>
      <span className="asset-copy"><strong>{item.name}</strong><small>{item.symbol} <i>·</i> {item.market}</small></span>
      <span className="open-arrow" aria-hidden="true">↗</span>
    </button>
    <div className="qoin-card-meta">
      <span className="qoin-meta-chip">{item.market}</span>
      <span className="qoin-meta-chip">{item.venue}</span>
      <span className="qoin-meta-chip">{item.providerStatus||"Provider status unknown"}</span>
      <span className="qoin-meta-chip qoin-price-chip">PRICE · {price!=null?price.toLocaleString(undefined,{maximumFractionDigits:8}):"—"}</span>
    </div>
    <div className="qoin-card-pager" aria-live="polite">
      {page===0?<div className="qoin-model-page">
        <div className="qoin-model-heading"><span className="qoin-card-kicker">RESEARCH MODEL 01</span><span className="qoin-signal-state">{decision?.action?.toUpperCase()||"AWAITING DATA"}</span></div>
        <div className="qoin-model-primary"><strong>{decision?.magnitude!=null?decision.magnitude.toFixed(4):"—"}</strong><span>Decision magnitude</span></div>
        <div className="qoin-model-vector"><span>Resultant vector · [bull, bear]</span><strong>{decision?.decision_vector?.[0]?.toFixed(4)??"—"} / {decision?.decision_vector?.[1]?.toFixed(4)??"—"}</strong></div>
        <div className="qoin-model-feature-grid">{QOIN_FEATURE_NAMES.map((name)=> <div key={name}><small>{name.replaceAll("_"," ")}</small><strong>{decision?.features?.[name]?.toFixed(3)??"—"}</strong></div>)}</div>
        <p className="qoin-model-reason">{decision?.reason||"No persisted vector decision is available yet. Values are not fabricated."}</p>
        <small className="qoin-model-time">{decision?.strategy_version||"Strategy version unavailable"} · {sourceTime?new Date(sourceTime).toLocaleString():"No source event timestamp"}</small>
      </div>:<div className="qoin-model-page">
        <div className="qoin-model-heading"><span className="qoin-card-kicker">RESEARCH MODEL 02</span><span className="qoin-signal-state">{ledgerStatus==="loaded"?"EVIDENCE LOADED":ledgerStatus==="loading"?"LOADING":ledgerStatus==="unavailable"?"UNAVAILABLE":"ON DEMAND"}</span></div>
        {ledger?<><div className="qoin-model-stat-grid">
          <div><small>Event universe</small><strong>{ledger.universe_count}</strong></div>
          <div><small>Evidence sets</small><strong>{ledger.sets.length}</strong></div>
          <div><small>Relations</small><strong>{ledger.relations.length}</strong></div>
          <div><small>History</small><strong>{ledger.durable_history_complete===false?"Bounded":"Reported"}</strong></div>
        </div>
        <div className="qoin-model-set-bars">{ledger.sets.slice(0,5).map((set)=> <div key={set.set_id}><span>{set.label}</span><i><b style={{width:ledger.universe_count?Math.min(100,set.measure.counting_measure/ledger.universe_count*100)+"%":"0%"}}/></i><strong>{set.measure.counting_measure}</strong></div>)}</div>
        <small className="qoin-model-time">{ledger.universe_truncated?"Sample capped; measures cover returned evidence only.":"Set memberships are derived from persisted source-event IDs."}</small>
        </>:<p className="qoin-model-reason">{ledgerStatus==="loading"?"Loading persisted evidence and set measures…":ledgerStatus==="unavailable"?"V2 evidence is unavailable for this instrument.":"Swipe to load set and ledger evidence."}</p>}
      </div>}
    </div>
    <div className="qoin-card-footer">
      <div className="qoin-page-dots" role="group" aria-label="Research model pages">
        <button type="button" aria-label="Show research model one" aria-pressed={page===0} className={page===0?"active":""} onClick={()=>setPage(0)}/>
        <button type="button" aria-label="Show research model two" aria-pressed={page===1} className={page===1?"active":""} onClick={()=>setPage(1)}/>
      </div>
      <span>SWIPE · TWO MODELS</span>
      <button type="button" className="text-action" aria-pressed={mode==="backtest"} onClick={onToggleMode}>{mode==="live"?"Backtest":"Live"}</button>
      {action&&<button className="text-action" onClick={action.onClick}>{action.label}</button>}
    </div>
  </article>;
}


const BROKER_DIRECTORY_PREVIEW = [
  { id:"binance_usdm", name:"Binance USD-M", category:"Execution adapter", status:"adapter_ready", detail:"USD-M perpetual execution adapter exists; authenticated account linking and multi-account dispatch are not connected.", selectable:true },
  { id:"coinbase_advanced_trade", name:"Coinbase Advanced Trade", category:"Market data only", status:"market_data_only", detail:"Public market stream is available. No order execution adapter is registered.", selectable:false },
  { id:"kraken_spot", name:"Kraken Spot", category:"Market data only", status:"market_data_only", detail:"Public market stream is available. No order execution adapter is registered.", selectable:false },
] as const;

function BrokerCollectionPanel() {
  const [selected,setSelected]=useState<string[]>([]);
  const [capital,setCapital]=useState("100");
  const [ready,setReady]=useState(false);
  useEffect(()=>{
    try {
      const raw=window.localStorage.getItem("qoin:broker-routing-preferences");
      if(raw){const parsed:unknown=JSON.parse(raw);if(Array.isArray(parsed))setSelected(parsed.filter((id):id is string=>typeof id==="string"&&BROKER_DIRECTORY_PREVIEW.some((broker)=>broker.id===id&&broker.selectable)).slice(0,15));}
    } catch { /* local preferences are optional */ }
    setReady(true);
  },[]);
  useEffect(()=>{if(ready)try{window.localStorage.setItem("qoin:broker-routing-preferences",JSON.stringify(selected));}catch{/* storage may be unavailable */}},[selected,ready]);
  const toggle=(id:string)=>{
    setSelected((current)=>current.includes(id)?current.filter((value)=>value!==id):current.length>=15?current:[...current,id]);
  };
  const amount=Number(capital);
  const valid=Number.isFinite(amount)&&amount>0;
  const perAccount=valid&&selected.length?amount/selected.length:null;
  return <section className="broker-collection" aria-label="Broker collection and routing preferences">
    <div className="coverage-heading"><div><p className="eyebrow">EXECUTION VENUES</p><h3>Broker collection</h3><p>Select eligible execution venues for a future strategy allocation. Market-data providers are not treated as brokers.</p></div><span className="coverage-status">{selected.length} SELECTED · MAX 15</span></div>
    <div className="broker-directory-grid">{BROKER_DIRECTORY_PREVIEW.map((broker)=><article className="broker-directory-card" key={broker.id}>
      <div className="broker-directory-top"><span className="broker-logo-mark">{broker.name.split(" ").map((part)=>part[0]).join("").slice(0,3)}</span><span className={"broker-status "+broker.status}>{broker.status.replaceAll("_"," ")}</span></div>
      <h3>{broker.name}</h3><p className="broker-market-label">{broker.category}</p><p className="broker-detail">{broker.detail}</p>
      <div className="broker-card-footer"><span>{broker.selectable?"Preference can be selected":"Not execution eligible"}</span>
        {broker.selectable?<button type="button" className={selected.includes(broker.id)?"broker-select selected":"broker-select"} aria-pressed={selected.includes(broker.id)} onClick={()=>toggle(broker.id)}>{selected.includes(broker.id)?"Selected":"Select"}</button>:<span aria-label="Unavailable for execution">—</span>}
      </div>
    </article>)}</div>
    <div className="broker-allocation-panel">
      <div className="broker-allocation-copy"><p className="eyebrow">ALLOCATION PREVIEW</p><h3>Equal target notional</h3><p>When multiple authenticated broker accounts are available, the strategy's parent notional can be divided equally across every selected eligible account. This is a preview only; the frontend does not submit or authorize orders.</p></div>
      <div className="broker-allocation-controls"><label htmlFor="qoin-allocation-capital">Portfolio notional</label><input id="qoin-allocation-capital" inputMode="decimal" value={capital} onChange={(event)=>setCapital(event.target.value)} aria-invalid={!valid}/><div className="broker-allocation-result"><span>Selected execution venues</span><strong>{selected.length}</strong><span>Target per selected account</span><strong>{perAccount==null?"—":perAccount.toLocaleString(undefined,{maximumFractionDigits:2})}</strong></div></div>
      <p className="broker-allocation-footnote">{selected.length===0?"Select an execution adapter to save a local routing preference. No broker accounts are connected yet.":selected.length===1?"One venue preference selected. Account linking and backend eligibility checks are still required.":"Equal target is calculated before broker quantity-step rounding. The backend must reject any split that fails minimums, margin, risk or cost checks."} The preference is stored on this device only; no API route currently persists broker selections.</p>
    </div>
  </section>;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("Archive");
  const [firebaseUser, setFirebaseUser] = useState<User | null>(null);
  const [authReady, setAuthReady] = useState(false);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [accountOpen, setAccountOpen] = useState(false);
  const [authPanelOpen, setAuthPanelOpen] = useState(false);
  const [profileOpen, setProfileOpen] = useState(false);
  const [authMode, setAuthMode] = useState<"signin" | "register">("signin");
  const [authEmail, setAuthEmail] = useState("");
  const [authPassword, setAuthPassword] = useState("");
  const [authError, setAuthError] = useState("");
  const [authBusy, setAuthBusy] = useState(false);
  const avatarInputRef = useRef<HTMLInputElement>(null);
  const archivedSymbolsRef = useRef<string[] | null>(null);
  const pendingArchiveActionRef = useRef<{ symbol: string; archived: boolean } | null>(null);
  const pendingInstrumentRef = useRef<Instrument | null>(null);
  const [instruments, setInstruments] = useState(seed);
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [mechanicsTab, setMechanicsTab] = useState<"vector" | "sets">("vector");
  const [routePath, setRoutePath] = useState(() => window.location.pathname);
  const [cardModeBySymbol, setCardModeBySymbol] = useState<Record<string, InstrumentMode>>(loadCardModes);
  const cardModesRef = useRef(cardModeBySymbol);
  const selectedMode: InstrumentMode = selected ? (cardModeBySymbol[selected.symbol] || "live") : "live";
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [health, setHealth] = useState<QoinHealth | null>(null);
  const [strategy, setStrategy] = useState<QoinStrategy | null>(null);
  const [catalogue, setCatalogue] = useState<QoinAlgorithmCatalogue | null>(null);
  const [catalogueStatus, setCatalogueStatus] = useState("loading");
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [vectorLoading, setVectorLoading] = useState(false);
  const [vectorError, setVectorError] = useState("");
  const [vectorHistory, setVectorHistory] = useState<QoinVectorHistory[]>([]);
  const [rawEvents, setRawEvents] = useState<QoinMarketEvent[]>([]);
  const [setLedger, setSetLedger] = useState<QoinSetLedger | null>(null);
  const [setLedgerStatus, setSetLedgerStatus] = useState("Waiting for instrument selection…");
  const [showRawEvents, setShowRawEvents] = useState(true);
  const [liveBySymbol, setLiveBySymbol] = useState<Record<string, { state?: QoinMarketState; decision?: QoinVectorDecision; event?: QoinStreamUpdate["event"] }>>({});
  const [marketState, setMarketState] = useState<QoinMarketState | null>(null);
  const [streamStatus, setStreamStatus] = useState("disconnected");
  const [lastEvent, setLastEvent] = useState<QoinStreamUpdate["event"] | null>(null);
  const [news, setNews] = useState<QoinNewsItem[]>([]);
  const [newsStatus, setNewsStatus] = useState("loading");
  const [newsCategory, setNewsCategory] = useState("general");
  const [macroEvents, setMacroEvents] = useState<QoinMacroEvent[]>([]);
  const [calendarStatus, setCalendarStatus] = useState("loading");
  const [notifications, setNotifications] = useState<QoinNotification[]>([]);
  const [notificationsOpen, setNotificationsOpen] = useState(false);
  const previousActionRef = useRef<Record<string, string>>({});
  const notifiedEventIdsRef = useRef<Set<string>>(new Set());
  useEffect(() => {
    let generation = 0;
    try {
      return subscribeQoinAuth((user) => {
        const requestGeneration = ++generation;
        setFirebaseUser(user);
        setAuthReady(true);
        setAvatarUrl(null);
        archivedSymbolsRef.current = null;
        if (!user) {
          setInstruments((items) => items.map((item) => ({ ...item, archived: ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol) })));
          return;
        }
        void getQoinArchivedSymbols().then((symbols) => {
          if (requestGeneration !== generation) return;
          const pendingAction = pendingArchiveActionRef.current;
          let nextSymbols = symbols;
          if (pendingAction?.archived && !symbols.includes(pendingAction.symbol)) nextSymbols = [...symbols, pendingAction.symbol];
          if (pendingAction && !pendingAction.archived) nextSymbols = symbols.filter((symbol) => symbol !== pendingAction.symbol);
          pendingArchiveActionRef.current = null;
          archivedSymbolsRef.current = nextSymbols;
          setInstruments((items) => items.map((item) => ({ ...item, archived: nextSymbols.includes(item.symbol) })));
          const pendingInstrument = pendingInstrumentRef.current;
          if (pendingInstrument) {
            pendingInstrumentRef.current = null;
            navigateToInstrument({ ...pendingInstrument, archived: nextSymbols.includes(pendingInstrument.symbol) }, cardModesRef.current[pendingInstrument.symbol] || "live");
          }
          if (pendingAction && nextSymbols !== symbols) {
            void saveQoinArchivedSymbols(nextSymbols).catch((error) => setAuthError(error instanceof Error ? error.message : "Could not save your archive."));
          }
        }).catch((error) => {
          if (requestGeneration === generation) setAuthError(error instanceof Error ? error.message : "Could not load your archive.");
        });
      });
    } catch {
      setFirebaseUser(null);
      setAuthError("Firebase is not configured. Add the VITE_FIREBASE_* values before signing in.");
      setAuthReady(true);
      return undefined;
    }
  }, []);
  function requireAuthentication() { setAccountOpen(true); setAuthPanelOpen(true); setAuthError(firebaseUser ? "" : "Sign in to use this action."); }
  function rememberCardMode(symbol: string, mode: InstrumentMode) {
    const next = { ...cardModesRef.current, [symbol]: mode };
    cardModesRef.current = next;
    setCardModeBySymbol(next);
    try { window.localStorage.setItem("qoin:archive-card-modes", JSON.stringify(next)); } catch { /* storage may be unavailable */ }
  }
  function navigateToInstrument(item: Instrument, mode: InstrumentMode) {
    const path = instrumentRoute(item, mode);
    window.history.pushState({ qoinInstrument: item.symbol, mode }, "", path);
    setRoutePath(path);
    setSelected(item);
    setMechanicsTab("vector");
    rememberCardMode(item.symbol, mode);
  }
  function closeInstrument() {
    if (window.location.pathname.startsWith("/app/instrument/")) {
      window.history.replaceState({}, "", "/app");
      setRoutePath("/app");
    }
    setSelected(null);
  }
  function toggleCardMode(item: Instrument) {
    rememberCardMode(item.symbol, (cardModeBySymbol[item.symbol] || "live") === "backtest" ? "live" : "backtest");
  }
  function openInstrument(item: Instrument) {
    if (!firebaseUser) { pendingInstrumentRef.current = item; requireAuthentication(); return; }
    setMechanicsTab("vector");
    navigateToInstrument(item, cardModeBySymbol[item.symbol] || "live");
  }
  function switchInstrumentMode(item: Instrument, mode: InstrumentMode) {
    rememberCardMode(item.symbol, mode);
    const path = instrumentRoute(item, mode);
    window.history.pushState({ qoinInstrument: item.symbol, mode }, "", path);
    setRoutePath(path);
  }
  useEffect(() => {
    const onPopState = () => setRoutePath(window.location.pathname);
    window.addEventListener("popstate", onPopState);
    return () => window.removeEventListener("popstate", onPopState);
  }, []);
  useEffect(() => {
    const match = routePath.match(/^\/app\/instrument\/([^/]+)\/(vector|backtest)\/?$/);
    if (!match) {
      if (!routePath.startsWith("/app/instrument/")) setSelected(null);
      return;
    }
    if (!firebaseUser) return;
    const slug = decodeURIComponent(match[1]).toUpperCase();
    const item = instruments.find((candidate) => instrumentSlug(candidate.symbol) === slug);
    if (!item) return;
    const mode: InstrumentMode = match[2] === "backtest" ? "backtest" : "live";
    rememberCardMode(item.symbol, mode);
    setSelected((current) => current?.symbol === item.symbol ? current : item);
  }, [routePath, instruments, firebaseUser]);
  async function submitAuthentication(event: FormEvent<HTMLFormElement>) {
    event.preventDefault(); setAuthError(""); setAuthBusy(true);
    try { if (authMode === "register") await registerQoinUser(authEmail, authPassword); else await signInQoinUser(authEmail, authPassword); setAuthPanelOpen(false); setAccountOpen(false); setAuthPassword(""); }
    catch (error) { setAuthError(error instanceof Error ? error.message : "Authentication failed."); }
    finally { setAuthBusy(false); }
  }
  async function changeAvatar(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0]; if (!file) return;
    try { const url = await uploadQoinAvatar(file); setAvatarUrl(url); setAuthError(""); }
    catch (error) { setAuthError(error instanceof Error ? error.message : "Avatar upload failed."); }
    finally { event.target.value = ""; }
  }
  const unreadNotifications = notifications.filter((item) => !item.read).length;
  function addNotification(notification: QoinNotification) {
    setNotifications((current) => current.some((item) => item.id === notification.id) ? current : [notification, ...current].slice(0, 100));
    if (typeof window !== "undefined" && "Notification" in window && Notification.permission === "granted") {
      try { new Notification(notification.title, { body: notification.body, tag: notification.id }); } catch { /* browser policy */ }
    }
  }

  useEffect(() => { getQoinHealth().then(setHealth).catch(() => setHealth(null)); getQoinStrategy().then(setStrategy).catch(() => setStrategy(null)); }, []);
  useEffect(() => { let active = true; getQoinAlgorithmCatalogue().then((data) => { if (active) { setCatalogue(data); setCatalogueStatus("loaded"); } }).catch(() => { if (active) setCatalogueStatus("unavailable"); }); return () => { active = false; }; }, []);
  useEffect(() => {
    let active = true;
    getQoinInstruments().then(({ items }) => {
      if (!active) return;
      const colors = ["#e6a23c", "#6477df", "#8b72e8", "#4d91d8", "#76a85a", "#8895a7"];
      setInstruments(items.map((item, index) => ({
        symbol: item.symbol, engineSymbol: item.engine_symbol, name: item.name,
        market: item.asset_class, description: item.description, venue: item.venue, engineVenue: item.venue_id || (item.asset_class === "Crypto" ? "binance_usdm" : "finnhub"),
        provider: item.provider, providerStatus: item.provider_status,
        archived: archivedSymbolsRef.current ? archivedSymbolsRef.current.includes(item.symbol) : ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol), color: colors[index % colors.length],
      })));
    }).catch(() => { if (active) setVectorError("Instrument registry unavailable; showing the local starter set."); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    let active = true;
    const load = () => {
      setNewsStatus("loading");
      getQoinNews(newsCategory).then((result) => {
        if (!active) return;
        setNews(result.items);
        setNewsStatus(result.status === "ok" ? "connected" : result.message || result.status);
      }).catch((error: unknown) => {
        if (active) setNewsStatus(error instanceof Error ? error.message : "News provider unavailable");
      });
    };
    load();
    const timer = setInterval(load, 5 * 60 * 1000);
    return () => { active = false; clearInterval(timer); };
  }, [newsCategory]);

  useEffect(() => {
    let active = true;
    const load = () => getQoinCalendar().then((result) => {
      if (!active) return;
      setMacroEvents(result.items);
      setCalendarStatus(result.status === "ok" ? "connected" : result.message || result.status);
    }).catch((error: unknown) => { if (active) setCalendarStatus(error instanceof Error ? error.message : "Calendar provider unavailable"); });
    load();
    const timer = setInterval(load, 15 * 60 * 1000);
    return () => { active = false; clearInterval(timer); };
  }, []);

  useEffect(() => {
    if (!selected) {
      setVector(null);
      setVectorError("");
      return;
    }
    let current = true;
    setVector(null);
    setVectorError("");
    setVectorLoading(true);
    getInstrumentVector(selected.engineSymbol).then((result) => {
      if (!current) return;
      if (result.decision_vector && result.features && result.attended_features) {
        setVector(result);
        setLiveBySymbol((current) => ({ ...current, [selected.engineSymbol.toUpperCase()]: { ...current[selected.engineSymbol.toUpperCase()], decision: result } }));
      } else {
        setVectorError(result.reason || "No persisted engine vector exists for this instrument yet.");
      }
    }).catch((error: unknown) => {
      if (current) setVectorError(error instanceof Error ? error.message : "Engine vector is unavailable.");
    }).finally(() => {
      if (current) setVectorLoading(false);
    });
    return () => { current = false; };
  }, [selected?.engineSymbol]);


  useEffect(() => {
    if (!selected) { setVectorHistory([]); setRawEvents([]); setSetLedger(null); setSetLedgerStatus("Waiting for instrument selection…"); setMarketState(null); setLastEvent(null); return; }
    let active = true;
    setVectorHistory([]);
    setRawEvents([]);
    setSetLedger(null);
    setSetLedgerStatus("Loading derived set memberships and temporal relations…");
    getQoinSetLedger(selected.engineSymbol, 1000).then((result) => { if (active) { setSetLedger(result); setSetLedgerStatus("Loaded"); } }).catch((error: unknown) => { if (active) setSetLedgerStatus(error instanceof Error ? error.message : "Set ledger unavailable."); });
    getInstrumentVectorHistory(selected.engineSymbol, 500).then((result) => { if (active) setVectorHistory(result.items); }).catch(() => undefined);
    getQoinEvents(selected.engineSymbol, 1000).then((result) => { if (active) setRawEvents(result.items.slice().reverse()); }).catch(() => undefined);
    return () => { active = false; };
  }, [selected?.engineSymbol]);

  useEffect(() => {
    const symbols = instruments.map((item) => item.engineSymbol);
    if (!symbols.length) return;
    let active = true;
    let ledgerRefreshTimer: ReturnType<typeof setTimeout> | null = null;
    let ledgerRefreshInFlight = false;
    let ledgerRefreshQueued = false;
    const refreshSetLedger = async () => {
      if (!active || !selected) return;
      if (ledgerRefreshInFlight) { ledgerRefreshQueued = true; return; }
      ledgerRefreshInFlight = true;
      try {
        const result = await getQoinSetLedger(selected.engineSymbol, 1000);
        if (active) {
          setSetLedger(result);
          setSetLedgerStatus("Live · " + new Date().toLocaleTimeString());
        }
      } catch (error) {
        if (active) setSetLedgerStatus(error instanceof Error ? "Refresh failed · " + error.message : "Set ledger refresh failed");
      } finally {
        ledgerRefreshInFlight = false;
        if (ledgerRefreshQueued && active) { ledgerRefreshQueued = false; void refreshSetLedger(); }
      }
    };
    const scheduleSetLedgerRefresh = () => {
      if (!selected || ledgerRefreshTimer) return;
      setSetLedgerStatus("New evidence · rebuilding V2 projection…");
      ledgerRefreshTimer = setTimeout(() => { ledgerRefreshTimer = null; void refreshSetLedger(); }, 450);
    };
    const unsubscribe = subscribeQoinStream(symbols, (update) => {
      if (!active) return;
      if (update.type === "market.update" && update.symbol) {
        const symbol = update.symbol.toUpperCase();
        if (update.decision?.action) {
          const action = update.decision.action.toUpperCase();
          const previous = previousActionRef.current[symbol];
          if ((action === "BUY" || action === "SELL") && previous !== action) {
            addNotification({ id: "signal-" + symbol + "-" + action + "-" + (update.server_ts_ms || Date.now()), symbol, title: symbol + " research signal: " + action, body: (update.decision.reason || "The engine decision changed.") + " Magnitude " + (update.decision.magnitude ?? 0).toFixed(3) + ". Research only; no order was placed.", kind: "signal", createdAt: update.server_ts_ms || Date.now(), read: false, details: JSON.stringify(update.decision, null, 2) });
          }
          previousActionRef.current[symbol] = action;
        }
        if (update.event && (update.event.event_type === "liquidation" || update.event.liquidation_side)) {
          const eventId = update.event.event_id || symbol + "-" + (update.event.ts_ms || update.server_ts_ms || Date.now());
          if (!notifiedEventIdsRef.current.has(eventId)) {
            notifiedEventIdsRef.current.add(eventId);
            addNotification({ id: "liquidation-" + eventId, symbol, title: symbol + " liquidation event", body: "Liquidation side: " + (update.event.liquidation_side || update.event.side || "not supplied") + ". Price: " + (update.event.price ?? "not supplied") + ".", kind: "liquidation", createdAt: update.event.ts_ms || update.server_ts_ms || Date.now(), read: false, details: JSON.stringify(update.event, null, 2) });
          }
        }
        setLiveBySymbol((current) => ({ ...current, [symbol]: { state: update.state, decision: update.decision, event: update.event } }));
        if (selected?.engineSymbol.toUpperCase() === symbol) {
          if (update.state) setMarketState(update.state);
          if (update.event) {
            setLastEvent(update.event);
            setRawEvents((current) => [{ ...update.event, event_id: update.event?.event_id || String(update.server_ts_ms || Date.now()), venue: update.event?.venue || "stream", symbol, event_type: update.event?.event_type || "unknown", ts_ms: update.event?.ts_ms || update.server_ts_ms || Date.now() } as QoinMarketEvent, ...current].slice(0, 1000));
          }
          if (update.event) scheduleSetLedgerRefresh();
          if (update.decision) {
            setVector(update.decision);
            setVectorHistory((current) => [...current.slice(-499), {
              symbol: update.decision!.symbol, decision_vector: update.decision!.decision_vector || [0, 0],
              magnitude: update.decision!.magnitude || 0, angle_radians: update.decision!.angle_radians || 0,
              action: update.decision!.action || "wait", strategy_version: update.decision!.strategy_version,
              stored_at: new Date(update.server_ts_ms || Date.now()).toISOString(),
            }]);
          }
        }
      }
      if (update.type === "stream.gap") {
        setStreamStatus("gap — resynchronizing snapshots");
        if (selected) {
          setSetLedgerStatus("Stream gap · restoring authoritative snapshots…");
          void Promise.all([
            getInstrumentVector(selected.engineSymbol),
            getInstrumentVectorHistory(selected.engineSymbol, 500),
            getQoinEvents(selected.engineSymbol, 1000),
            getQoinSetLedger(selected.engineSymbol, 1000),
          ]).then(([decision, history, events, ledger]) => {
            if (!active) return;
            if (decision.decision_vector && decision.features && decision.attended_features) setVector(decision);
            setVectorHistory(history.items);
            setRawEvents(events.items.slice().reverse());
            setSetLedger(ledger);
            setSetLedgerStatus("Resynchronized · " + new Date().toLocaleTimeString());
          }).catch((error: unknown) => {
            if (active) setSetLedgerStatus(error instanceof Error ? "Resynchronization failed · " + error.message : "Resynchronization failed");
          });
        }
      }
    }, setStreamStatus);
    return () => { active = false; if (ledgerRefreshTimer) clearTimeout(ledgerRefreshTimer); unsubscribe(); };
  }, [instruments, selected?.engineSymbol]);

  useEffect(() => {
    const current = selected ? liveBySymbol[selected.engineSymbol.toUpperCase()] : undefined;
    setMarketState(current?.state || null);
    setLastEvent(current?.event || null);
  }, [selected?.engineSymbol, liveBySymbol]);

  const archived = useMemo(() => instruments.filter((item) => item.archived), [instruments]);
  const decisionStrategy = vector?.strategy_parameters || strategy;
  const discovered = useMemo(() => instruments.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [instruments, market, query]);

  function toggleArchive(symbol: string) {
    const target = instruments.find((item) => item.symbol === symbol);
    if (!target) return;
    if (!firebaseUser) {
      pendingArchiveActionRef.current = { symbol, archived: !target.archived };
      requireAuthentication();
      return;
    }
    if (archivedSymbolsRef.current === null) {
      setAuthError("Your archive is still syncing. Please try again in a moment.");
      return;
    }
    const previousSymbols = archivedSymbolsRef.current;
    const nextSymbols = target.archived ? previousSymbols.filter((value) => value !== symbol) : [...previousSymbols, symbol];
    archivedSymbolsRef.current = nextSymbols;
    setInstruments((items) => items.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
    void saveQoinArchivedSymbols(nextSymbols).catch((error) => {
      if (archivedSymbolsRef.current === nextSymbols) {
        archivedSymbolsRef.current = previousSymbols;
        setInstruments((items) => items.map((item) => ({ ...item, archived: previousSymbols.includes(item.symbol) })));
        setSelected((current) => current?.symbol === symbol ? { ...current, archived: target.archived } : current);
      }
      setAuthError(error instanceof Error ? error.message : "Could not save your archive.");
    });
  }

  if (!authReady) {
    return <main className="auth-gate" aria-live="polite"><section className="auth-gate-card"><span className="auth-gate-mark">q</span><p className="eyebrow">PRIVATE MARKET WORKSPACE</p><h1>Welcome to Qoin.</h1><p className="auth-gate-copy">Sign in to open your Archive, explore markets, and inspect the research behind each instrument.</p><div className="auth-gate-loading"><span /> Checking your session…</div></section></main>;
  }

  if (!firebaseUser) {
    return (
      <main className="auth-gate">
        <section className="auth-gate-card">
          <span className="auth-gate-mark">q</span>
          <p className="eyebrow">YOUR MARKET WORKSPACE</p>
          <h1>{authMode === "register" ? "Create your Qoin account." : "Welcome back."}</h1>
          <p className="auth-gate-copy">Your Archive, selected instruments, and research workspace are available after you sign in.</p>
          <form className="auth-gate-form" onSubmit={submitAuthentication}>
            <label>Email<input type="email" autoComplete="email" value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} placeholder="you@example.com" required /></label>
            <label>Password<input type="password" autoComplete={authMode === "register" ? "new-password" : "current-password"} value={authPassword} onChange={(event) => setAuthPassword(event.target.value)} minLength={6} required /></label>
            {authError && <p role="alert" className="auth-gate-error">{authError}</p>}
            <button className="primary-button" disabled={authBusy}>{authBusy ? "Please wait…" : authMode === "register" ? "Create account" : "Sign in"}</button>
          </form>
          <button type="button" className="auth-gate-switch" onClick={() => { setAuthMode((value) => value === "signin" ? "register" : "signin"); setAuthError(""); }}>{authMode === "register" ? "Already have an account? Sign in" : "New to Qoin? Create an account"}</button>
          <p className="auth-gate-footnote">Secure account access powered by Firebase Authentication.</p>
        </section>
      </main>
    );
  }

  return (
    <div className="qoin-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Qoin home"><span className="brand-mark">q</span><span>qoin</span></a>
        <div className="topbar-right"><span className={"connection-dot " + (health ? "connected" : "")} /><span>{health ? "Research engine connected" : "Research workspace"}</span><nav className="desktop-header-navigation" aria-label="Workspace"><button type="button" className={tab === "Archive" ? "desktop-header-nav-item active" : "desktop-header-nav-item"} aria-label="Archive" title="Archive" aria-current={tab === "Archive" ? "page" : undefined} onClick={() => { setTab("Archive"); closeInstrument(); }}><svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7.5h16v12H4zM3 4.5h18v3H3zM9 11.5h6" /></svg></button><button type="button" className={tab === "Explore" ? "desktop-header-nav-item active" : "desktop-header-nav-item"} aria-label="Explore" title="Explore" aria-current={tab === "Explore" ? "page" : undefined} onClick={() => { setTab("Explore"); closeInstrument(); }}><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="10.8" cy="10.8" r="6.3" /><path d="m15.5 15.5 4.2 4.2M8.2 13.2l2.1-4.8 2.9 2.8z" /></svg></button></nav><button className="avatar" aria-label="Account menu" aria-expanded={accountOpen} onClick={() => setAccountOpen((value) => !value)}>{(avatarUrl || firebaseUser?.photoURL) ? <img src={avatarUrl || firebaseUser?.photoURL || ""} alt="" /> : (firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</button></div>
        {accountOpen && <section className="account-menu" aria-label="Account menu">
          <div className="account-identity"><strong>{firebaseUser?.displayName || firebaseUser?.email || "Guest"}</strong><small>{firebaseUser ? "Signed in with Firebase" : "Not signed in"}</small></div>
          {firebaseUser ? <>
            <button onClick={() => { setAccountOpen(false); setProfileOpen((value) => !value); setAuthPanelOpen(false); }}>Profile</button>
            <button onClick={() => { setNotificationsOpen((value) => !value); setAccountOpen(false); }}>Notifications <span>{unreadNotifications ? unreadNotifications : ""}</span></button>
            <button onClick={() => avatarInputRef.current?.click()}>Change profile picture</button>
            <button onClick={() => { void signOutQoinUser().then(() => { setAccountOpen(false); setProfileOpen(false); setNotificationsOpen(false); setAuthError(""); }).catch((error: unknown) => setAuthError(error instanceof Error ? error.message : "Sign out failed. Try again.")); }}>Sign out</button>
          </> : <>
            <button onClick={() => { setAuthMode("signin"); setAuthPanelOpen(true); }}>Sign in</button>
            <button onClick={() => { setAuthMode("register"); setAuthPanelOpen(true); }}>Create account</button>
          </>}
        </section>}
        <input ref={avatarInputRef} className="visually-hidden" type="file" accept="image/jpeg,image/png,image/webp" onChange={changeAvatar} />
        {profileOpen && firebaseUser && <section className="account-menu profile-menu"><div className="account-identity"><strong>{firebaseUser.displayName || "Profile"}</strong><small>{firebaseUser.email}</small><small>{firebaseUser.uid}</small></div><button onClick={() => { setProfileOpen(false); setAccountOpen(false); }}>Close</button></section>}
        {authPanelOpen && <section className="auth-panel"><form onSubmit={submitAuthentication}><div className="account-identity"><strong>{authMode === "register" ? "Create your Qoin account" : "Sign in to Qoin"}</strong><small>Firebase-secured account</small></div><label>Email<input type="email" autoComplete="email" value={authEmail} onChange={(event) => setAuthEmail(event.target.value)} required /></label><label>Password<input type="password" autoComplete={authMode === "register" ? "new-password" : "current-password"} value={authPassword} onChange={(event) => setAuthPassword(event.target.value)} minLength={6} required /></label>{authError && <p role="alert">{authError}</p>}<button className="primary-button" disabled={authBusy}>{authBusy ? "Please wait…" : authMode === "register" ? "Create account" : "Sign in"}</button><button type="button" className="account-switch" onClick={() => { setAuthMode((value) => value === "signin" ? "register" : "signin"); setAuthError(""); }}>{authMode === "register" ? "Already registered? Sign in" : "New to Qoin? Create an account"}</button><button type="button" className="account-switch" onClick={() => setAuthPanelOpen(false)}>Close</button></form></section>}
        {notificationsOpen && <section className="notification-panel" aria-label="Notification centre">
          <div className="notification-panel-heading"><div><strong>Notifications</strong><small>{unreadNotifications} unread · live engine events</small></div><button onClick={() => setNotifications((current) => current.map((item) => ({ ...item, read: true })))}>Mark read</button></div>
          {typeof window !== "undefined" && "Notification" in window && Notification.permission !== "granted" && <button className="notification-permission" onClick={() => { if ("Notification" in window) void Notification.requestPermission(); }}>Enable desktop alerts</button>}
          <div className="notification-list">{notifications.length ? notifications.map((item) => <article className={"notification-item " + (item.read ? "is-read" : "")} key={item.id}><div className="notification-item-top"><span>{item.kind.toUpperCase()} · {item.symbol}</span><time>{new Date(item.createdAt).toLocaleTimeString()}</time></div><strong>{item.title}</strong><p>{item.body}</p><details><summary>Inspect trigger payload</summary><pre>{item.details}</pre></details></article>) : <p className="notification-empty">No alerts yet. Signal transitions and liquidation events from the live stream will appear here.</p>}</div>
        </section>}
      </header>






      <div className="workspace qoin-workspace">
      <aside className="desktop-profile-sidebar" aria-label="Qoin workspace navigation">
        <div className="desktop-sidebar-profile">
          <button className="desktop-profile-avatar" onClick={() => avatarInputRef.current?.click()} aria-label="Change profile picture">
            {(avatarUrl || firebaseUser?.photoURL) ? <img src={avatarUrl || firebaseUser?.photoURL || ""} alt="" /> : (firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}
            <span aria-hidden="true">+</span>
          </button>
          <strong>{firebaseUser?.displayName || firebaseUser?.email?.split("@")[0] || "Qoin member"}</strong>
          <small>{firebaseUser?.email}</small>
          <button className="desktop-profile-link" onClick={() => setProfileOpen((value) => !value)}>View profile</button>
          {profileOpen && <div className="desktop-profile-details"><span>Account</span><strong>{firebaseUser?.email}</strong><small>Signed in securely with Firebase</small></div>}
        </div>
        <div className="desktop-sidebar-bottom">
          <button onClick={() => setNotificationsOpen((value) => !value)}><span aria-hidden="true">♧</span>Notifications{unreadNotifications > 0 && <small>{unreadNotifications}</small>}</button>
          <button onClick={() => avatarInputRef.current?.click()}><span aria-hidden="true">◉</span>Change photo</button>
          <button onClick={() => { void signOutQoinUser().catch((error: unknown) => setAuthError(error instanceof Error ? error.message : "Sign out failed. Try again.")); }}><span aria-hidden="true">↗</span>Sign out</button>
        </div>
      </aside>
        <main className="home-columns qoin-home-columns" aria-label="Qoin workspace">
          <section className={`home-column archive-column ${tab === "Archive" ? "mobile-tab-active" : "mobile-tab-hidden"}`} aria-label="Archive">
            <section className="content-section">
                      <div className="section-heading"><div><p className="eyebrow">YOUR COLLECTION</p><h2>Archive</h2><p>Every instrument you follow, in one place.</p></div><span className="count-label">{archived.length} INSTRUMENTS</span></div>
                      {archived.length ? <div className="archive-workspace">
                        <div className="instrument-grid">{archived.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => openInstrument(item)} mode={cardModeBySymbol[item.symbol] || "live"} onToggleMode={() => toggleCardMode(item)} live={liveBySymbol[item.engineSymbol.toUpperCase()]} />)}</div>
                        <aside className="archive-market-sidebar" aria-label="Market pulse">
                          <div className="market-pulse-panel">
                            <div className="market-pulse-heading"><strong>Market pulse</strong><span>OBSERVED DATA</span></div>
                            <p className="market-pulse-copy">Latest provider observations for instruments in your Archive.</p>
                            {archived.slice(0, 5).map((item) => {
                              const snapshot = liveBySymbol[item.engineSymbol.toUpperCase()];
                              return <div className="market-pulse-row" key={item.symbol}>
                                <span><strong>{item.symbol}</strong><small>{item.name}</small></span>
                                <span><strong>{snapshot?.state?.last_price != null ? snapshot.state.last_price.toLocaleString(undefined,{maximumFractionDigits:8}) : "—"}</strong><small>{snapshot?.decision?.action?.toUpperCase() || "NO LIVE DATA"}</small></span>
                              </div>;
                            })}
                            <p className="market-pulse-footnote">Unavailable or stale provider values remain blank; Qoin does not synthesize prices.</p>
                          </div>
                        </aside>
                      </div> : <div className="empty-state"><strong>Your archive is clear.</strong><p>Explore instruments and add the ones you want to follow.</p><button className="primary-button" onClick={() => setTab("Explore")}>Explore instruments</button></div>}
                    </section>
          </section>
          <section className={`home-column explore-column ${tab === "Explore" ? "mobile-tab-active" : "mobile-tab-hidden"}`} aria-label="Explore">
            <section className="content-section">
                      <div className="explore-workspace">
                      <div className="explore-main-column">
                      <div className="section-heading"><div><p className="eyebrow">DISCOVERY</p><h2>Explore</h2><p>Find instruments to bring into your archive.</p></div><span className="count-label">CRYPTO · EQUITIES · FX · INDICES · COMMODITIES</span></div>
                      <label className="search-box"><span>⌕</span><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search instruments or symbols" /></label>
                      <div className="filter-row">{["All", "Crypto", "Equities", "FX", "Indices", "Commodities"].map((name) => <button key={name} className={market === name ? "filter active" : "filter"} onClick={() => setMarket(name)}>{name}</button>)}</div>
                      <section className="coverage-preview" aria-label="Market access tier preview">
                        <div className="coverage-heading"><div><p className="eyebrow">MARKET ACCESS DESIGN</p><h3>Coverage tiers</h3><p>Market-data breadth is separate from algorithm performance and broker execution.</p></div><span className="coverage-status">{catalogueStatus === "loaded" ? (catalogue?.market_access_tiers_enabled ? "ACCESS ENABLED" : "PREVIEW ONLY") : catalogueStatus === "loading" ? "LOADING CATALOGUE" : "CATALOGUE UNAVAILABLE"}</span></div>
                        {catalogue?.market_access_tiers?.length ? <div className="coverage-grid">{catalogue.market_access_tiers.map((tier) => <article className="coverage-card" key={tier.tier_id}>
                          <div className="coverage-card-top"><strong>{tier.display_name}</strong><span>{tier.status}</span></div>
                          <div className="coverage-primary"><b>{tier.max_active_instruments}</b><span>active instruments</span></div>
                          <dl><div><dt>Markets</dt><dd>{tier.asset_classes.join(" · ")}</dd></div><div><dt>History</dt><dd>{tier.historical_depth_days} days</dd></div><div><dt>Timeframes</dt><dd>{tier.supported_timeframes.join(" · ")}</dd></div><div><dt>Data providers</dt><dd>{tier.market_data_providers.join(" · ") || "None listed"}</dd></div><div><dt>Execution brokers</dt><dd>{tier.execution_brokers.length ? tier.execution_brokers.join(" · ") : "None — research only"}</dd></div></dl>
                          <p>{tier.notes}</p>
                        </article>)}</div> : <p className="coverage-unavailable">{catalogueStatus === "loading" ? "Loading proposed market coverage…" : "Market coverage catalogue could not be loaded. Instrument availability remains provider-dependent."}</p>}
                        <p className="coverage-disclaimer">These are proposed coverage limits, not active subscriptions, prices or enforced permissions. Finnhub coverage requires a configured key and eligible data; no live broker execution is connected.</p>
                      </section>
                      <BrokerCollectionPanel />
                      <section className="algorithm-catalogue" aria-label="Research algorithm catalogue">
                        <div className="coverage-heading"><div><p className="eyebrow">STRATEGY LIBRARY</p><h3>Research algorithms</h3><p>Independent models, explicit versions and declared market requirements.</p></div><span className="coverage-status">{catalogue?.selection_enabled ? "SELECTION ENABLED" : "READ-ONLY CATALOGUE"}</span></div>
                        {catalogue?.algorithms?.length ? <div className="algorithm-grid">{catalogue.algorithms.map((algorithm) => <article className="algorithm-card" key={algorithm.algorithm_id}>
                          <div className="algorithm-card-top"><div><strong>{algorithm.display_name}</strong><small>{algorithm.algorithm_id} · {algorithm.version}</small></div><span className={"algorithm-status " + algorithm.status}>{algorithm.status}</span></div>
                          <p>{algorithm.description}</p>
                          <dl><div><dt>Markets</dt><dd>{algorithm.supported_asset_classes.join(" · ") || "Not declared"}</dd></div><div><dt>Venues</dt><dd>{algorithm.supported_venues.join(" · ") || "Not declared"}</dd></div><div><dt>Timeframes</dt><dd>{algorithm.supported_timeframes.join(" · ") || "Not declared"}</dd></div><div><dt>Data providers</dt><dd>{algorithm.supported_market_data_providers.join(" · ") || "Not declared"}</dd></div><div><dt>Required features</dt><dd>{algorithm.required_features.join(" · ") || "None declared"}</dd></div><div><dt>Risk profile</dt><dd>{algorithm.risk_profile}</dd></div></dl>
                          <div className="algorithm-card-footer"><span>Validation: {algorithm.evaluation_status.replaceAll("_", " ")}</span><span>{algorithm.execution_mode.replaceAll("_", " ")}</span></div>
                        </article>)}</div> : <p className="coverage-unavailable">{catalogueStatus === "loading" ? "Loading algorithm manifests…" : "No algorithm manifests are currently published by the engine."}</p>}
                        <p className="coverage-disclaimer">{catalogue?.note || "Algorithm selection and paid entitlements are not assumed. A manifest describes capabilities; it does not prove independent validation, profitable performance or user access."}</p>
                      </section>
                      {discovered.length ? <div className="instrument-grid">{discovered.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => openInstrument(item)} mode={cardModeBySymbol[item.symbol] || "live"} onToggleMode={() => toggleCardMode(item)} live={liveBySymbol[item.engineSymbol.toUpperCase()]} action={{ label: "＋ Add to archive", onClick: () => toggleArchive(item.symbol) }} />)}</div> : <div className="empty-state"><strong>No instruments found.</strong><p>Try another name or market.</p></div>}
                      <section className="news-section">
                        <div className="news-heading"><div><p className="eyebrow">GLOBAL FINANCIAL INTELLIGENCE</p><h3>News & macro events</h3><p>Provider-attributed headlines. Publication timestamps and original links are preserved.</p></div><span className="news-status">{newsStatus}</span></div>
                        <div className="filter-row">{["general", "forex", "crypto", "merger"].map((category) => <button key={category} className={newsCategory === category ? "filter active" : "filter"} onClick={() => setNewsCategory(category)}>{category}</button>)}</div>
                        {news.length ? <div className="news-list">{news.map((item) => <article className="news-item" key={String(item.id)}><div className="news-meta"><span>{item.source}</span><span>{item.published_at_ms ? new Date(item.published_at_ms).toLocaleString() : "Timestamp unavailable"}</span></div><a href={item.url} target="_blank" rel="noreferrer">{item.headline}</a><p>{item.summary}</p>{item.related && <small>Related: {item.related}</small>}</article>)}</div> : <p className="provider-empty">{newsStatus === "loading" ? "Loading sourced headlines…" : newsStatus + ". Configure FINNHUB_API_KEY on the backend to load live news. No placeholder headlines are generated."}</p>}
                      </section>
                      <section className="macro-section">
                        <div className="news-heading"><div><p className="eyebrow">UPCOMING CATALYSTS</p><h3>Economic calendar</h3><p>Scheduled releases from the configured provider, with actual, estimate and previous values where published.</p></div><span className="news-status">{calendarStatus}</span></div>
                        {macroEvents.length ? <div className="macro-list">{macroEvents.map((item,index) => <article className="macro-item" key={item.time + item.country + item.event + index}>
                          <div><strong>{item.event}</strong><small>{item.country} · {item.time} · {item.impact || "Impact not supplied"}</small></div>
                          <div className="macro-values"><span>Actual <b>{item.actual ?? "—"}</b></span><span>Estimate <b>{item.estimate ?? "—"}</b></span><span>Previous <b>{item.previous ?? "—"}</b></span></div>
                        </article>)}</div> : <p className="provider-empty">{calendarStatus === "loading" ? "Loading economic calendar…" : calendarStatus + ". Configure FINNHUB_API_KEY to load macro events. No event dates are fabricated."}</p>}
                      </section>
                      </div>
                      <aside className="explore-market-sidebar" aria-label="Your market watchlist">
                        <div className="market-pulse-panel">
                          <div className="market-pulse-heading"><strong>Your watchlist</strong><span>{archived.length} SAVED</span></div>
                          <p className="market-pulse-copy">Live provider snapshots for instruments already in your Archive.</p>
                          {archived.length ? archived.slice(0, 8).map((item) => {
                            const snapshot = liveBySymbol[item.engineSymbol.toUpperCase()];
                            return <button className="market-pulse-row explore-watch-row" key={item.symbol} onClick={() => openInstrument(item)}>
                              <span><strong>{item.symbol}</strong><small>{item.name}</small></span>
                              <span><strong>{snapshot?.state?.last_price != null ? snapshot.state.last_price.toLocaleString(undefined,{maximumFractionDigits:8}) : "—"}</strong><small>{snapshot?.decision?.action?.toUpperCase() || "NO LIVE DATA"}</small></span>
                            </button>;
                          }) : <p className="market-pulse-footnote">Add an instrument to your Archive to build a watchlist.</p>}
                          <p className="market-pulse-footnote">Unavailable or stale provider values remain blank. Qoin never fabricates a quote.</p>
                        </div>
                        <div className="market-pulse-panel explore-engine-panel">
                          <div className="market-pulse-heading"><strong>Research engine</strong><span>{health ? "CONNECTED" : "CHECKING"}</span></div>
                          <p className="market-pulse-copy">Research and historical replay only. No live order execution is available.</p>
                        </div>
                      </aside>
                      </div>
                    </section>
          </section>
        </main>
      </div>

      <footer className="footer"><span>QOIN · MARKET INTELLIGENCE</span><span>RESEARCH MODE · NO LIVE EXECUTION</span></footer>

      <nav className="floating-navigation" aria-label="Primary navigation">
        <button type="button" className={tab === "Archive" ? "floating-nav-item active" : "floating-nav-item"} aria-label="Archive" title="Archive" aria-current={tab === "Archive" ? "page" : undefined} onClick={() => { setTab("Archive"); closeInstrument(); }}><span className="floating-nav-icon" aria-hidden="true">◈</span></button>
        <button type="button" className={tab === "Explore" ? "floating-nav-item active" : "floating-nav-item"} aria-label="Explore" title="Explore" aria-current={tab === "Explore" ? "page" : undefined} onClick={() => { setTab("Explore"); closeInstrument(); }}><span className="floating-nav-icon" aria-hidden="true">⌕</span></button>
      </nav>



      {selected && <div className="modal-backdrop" role="presentation" onMouseDown={(e) => { if (e.target === e.currentTarget) closeInstrument(); }}><section className="vector-modal mechanics-modal" role="dialog" aria-modal="true" aria-labelledby="vector-title">
        <header className="modal-header"><div><p className="eyebrow">{selectedMode === "backtest" ? "BACKTEST ARENA" : "INSTRUMENT SPACE"}</p><h2 id="vector-title">{selectedMode === "backtest" ? selected.name + " · Backtest" : selected.name}</h2><p>{selected.symbol} <i>·</i> {selected.market}</p></div><div style={{ display: "flex", alignItems: "center", gap: 8 }}><button className="text-action" onClick={() => switchInstrumentMode(selected, selectedMode === "live" ? "backtest" : "live")}>{selectedMode === "live" ? "Open Backtest Arena" : "Return to Live"}</button><button className="close-button" onClick={closeInstrument} aria-label="Close instrument workspace">×</button></div></header>
        <div className="modal-body">
          {selectedMode === "live" && <div className="mechanics-tabs" role="tablist" aria-label="Instrument mechanics"><button type="button" role="tab" aria-selected={mechanicsTab === "vector"} className={mechanicsTab === "vector" ? "active" : ""} onClick={() => setMechanicsTab("vector")}>Vector space</button><button type="button" role="tab" aria-selected={mechanicsTab === "sets"} className={mechanicsTab === "sets" ? "active" : ""} onClick={() => setMechanicsTab("sets")}>Sets &amp; Ledger</button></div>}
          {selectedMode === "backtest" ? <section className="backtest-arena" style={{ display: "grid", gap: 16 }}>
            <div className="vector-intro"><div><strong>Backtest mechanics</strong><p>Replay the selected instrument's stored observations through the pinned V1 strategy. Every simulated decision and fill remains inspectable.</p></div><span className="preview-tag">RESEARCH ONLY</span></div>
            <section className="backtest-method-panel" aria-label="Historical replay mechanics">
              <div className="attention-heading"><strong>How the replay progresses</strong><span>Deterministic · event-time ordered</span></div>
              <ol>
                <li><strong>Build the historical tape.</strong> Load persisted events for this instrument and selected venue, ordered by event timestamp and event ID.</li>
                <li><strong>Reconstruct state.</strong> Feed each observation through the same V1 state and feature calculations used by the live research engine.</li>
                <li><strong>Evaluate decisions.</strong> Apply the recorded strategy version and its gates; BUY, SELL and WAIT decisions are counted separately.</li>
                <li><strong>Simulate execution.</strong> A signal can fill only on a strictly later eligible trade or book event. Book buys use ask and sells use bid; latency and liquidity participation constrain fills.</li>
                <li><strong>Account for costs.</strong> Fees, slippage, market impact and recorded funding cashflows flow into equity, realized P/L and drawdown.</li>
                <li><strong>Audit the result.</strong> Inspect fills, pending orders, raw-event gaps, assumptions and cost-stress scenarios. No broker order is submitted.</li>
              </ol>
              <p className="disclaimer">This is a replay of events already stored by Qoin, not a complete historical market-data download. Missing observations remain missing; the replay reports gaps rather than inventing candles or prices.</p>
            </section>
            <VectorMap decision={vector} history={vectorHistory} />
            <FeatureSpace decision={vector} />
            {vector ? <DecisionTracePanel decision={vector} /> : <p className="disclaimer">{vectorError || "No persisted decision is available yet. Backtest replay uses stored raw events and the active Rust vector strategy."}</p>}
            <ResearchLedger decision={vector} history={vectorHistory} events={rawEvents} />
            <SetLedgerPanel ledger={setLedger} status={setLedgerStatus} />
            <BacktestPanel symbol={selected.engineSymbol} venue={selected.engineVenue} authenticated={Boolean(firebaseUser)} onRequireAuth={requireAuthentication} />
          </section> : mechanicsTab === "vector" ? <>
          <div className="vector-intro"><div><strong>Vector space</strong><p>Actual six-dimensional engine features, attention interactions and connected decision output.</p></div><span className="preview-tag">{streamStatus.toUpperCase()}</span></div>
          <VectorSpaceMechanics decision={vector} />
          <div className="vector-intro"><div><strong>Resultant decision space</strong><p>Persisted bullish and bearish decision-vector trajectory.</p></div></div>
          <VectorMap decision={vector} history={vectorHistory} />
          <FeatureSpace decision={vector} />
          <section className="live-state-panel">
            <div className="live-state-heading"><strong>Live market state</strong><span>{lastEvent?.event_type || "awaiting event"}</span></div>
            <div className="live-state-grid">
              <div><span>Last price</span><strong>{marketState?.last_price?.toLocaleString(undefined,{maximumFractionDigits:8}) ?? "—"}</strong></div>
              <div><span>Spread (bps)</span><strong>{marketState?.spread_bps?.toFixed(3) ?? "—"}</strong></div>
              <div><span>Signed flow</span><strong>{marketState?.signed_flow?.toFixed(4) ?? "—"}</strong></div>
              <div><span>Order flow imbalance</span><strong>{marketState?.order_flow_imbalance?.toFixed(4) ?? "—"}</strong></div>
              <div><span>Funding rate</span><strong>{marketState?.funding_rate?.toFixed(6) ?? "—"}</strong></div>
              <div><span>Open interest</span><strong>{marketState?.open_interest?.toLocaleString() ?? "—"}</strong></div>
              <div><span>Liquidations</span><strong>{marketState?.liquidation_events ?? "—"}</strong></div>
              <div><span>Last liquidation side</span><strong>{marketState?.last_liquidation_side || "—"}</strong></div>
              <div><span>Event count</span><strong>{marketState?.event_count ?? "—"}</strong></div>
              <div><span>Total trade size</span><strong>{marketState?.total_trade_size?.toFixed(4) ?? "—"}</strong></div>
              <div><span>Event intensity</span><strong>{marketState?.event_intensity?.toFixed(3) ?? "—"}</strong></div>
              <div><span>EWMA abs return</span><strong>{marketState?.ewma_abs_return?.toFixed(6) ?? "—"}</strong></div>
              <div><span>Last event time</span><strong>{marketState?.last_event_ts_ms ? new Date(marketState.last_event_ts_ms).toLocaleTimeString() : "—"}</strong></div>
              <div><span>State quality</span><strong>{marketState?.state_quality?.toFixed(3) ?? "—"}</strong></div>
            </div>
            <p className="disclaimer">A dash means the provider has not supplied that field. Last raw event: {lastEvent ? JSON.stringify(lastEvent) : "none received"}.</p>
          </section>
          {vectorLoading ? <p className="disclaimer">Loading the latest persisted engine vector…</p> : vector ? (
            <>
              <div className="engine-metrics">
                <div><span>Decision</span><strong>{vector.action?.toUpperCase() || "WAIT"}</strong></div>
                <div><span>Magnitude</span><strong>{vector.magnitude?.toFixed(3) ?? "—"}</strong></div>
                <div><span>Angle</span><strong>{vector.angle_radians?.toFixed(3) ?? "—"} rad</strong></div>
                <div><span>Strategy</span><strong>{vector.strategy_version || "—"}</strong></div>
                <div><span>Bullish component</span><strong>{vector.decision_vector?.[0].toFixed(3) ?? "—"}</strong></div>
                <div><span>Bearish component</span><strong>{vector.decision_vector?.[1].toFixed(3) ?? "—"}</strong></div>
              </div>
              <div className="dimension-list">
                {QOIN_FEATURE_NAMES.map((name, index) => {
                  const raw = vector.features?.[name];
                  const attended = vector.attended_features?.[index];
                  const weights = vector.attention?.[index] ?? [];
                  const strongest = weights.length ? weights.indexOf(Math.max(...weights)) : -1;
                  return <div className="engine-feature" key={name}>
                    <span>{name.replaceAll("_", " ")}</span>
                    <strong>{raw?.toFixed(3) ?? "—"} → {attended?.toFixed(3) ?? "—"}</strong>
                    {strongest >= 0 && <small>Strongest attention: {QOIN_FEATURE_NAMES[strongest]} ({weights[strongest].toFixed(2)})</small>}
                  </div>;
                })}
              </div>
              <section className="contribution-panel">
                <div className="attention-heading"><strong>Feature contribution ledger</strong><span>Coefficient × attended feature</span></div>
                {QOIN_FEATURE_NAMES.map((name,index) => {
                  const attended = vector.attended_features?.[index] ?? 0;
                  const trace = vector.feature_trace?.find((item) => item.name === name);
                  const bullishWeight = trace?.bullish_weight;
                  const bearishWeight = trace?.bearish_weight;
                  return <div className="contribution-row" key={name}>
                    <strong>{name.replaceAll("_"," ")}</strong>
                    <span>{bullishWeight == null ? "Bullish weight unavailable" : <>Bull {bullishWeight.toFixed(3)} × {attended.toFixed(3)} = <b>{(bullishWeight*attended).toFixed(4)}</b></>}</span>
                    <span>{bearishWeight == null ? "Bearish weight unavailable" : <>Bear {bearishWeight.toFixed(3)} × {attended.toFixed(3)} = <b>{(bearishWeight*attended).toFixed(4)}</b></>}</span>
                  </div>;
                })}
                <p className="disclaimer">Signed contributions are shown before the strategy's aggregate positive clamp. The actual decision vector above remains authoritative.</p>
              </section>
              <DecisionTracePanel decision={vector} />

              <section className="attention-panel" aria-label="Six by six attention weight matrix">
                <div className="attention-heading"><strong>Attention matrix</strong><span>Each row shows how one feature attends to the others</span></div>
                <div className="attention-grid">
                  <span className="attention-corner" />
                  {QOIN_FEATURE_SHORT_NAMES.map((name) => <span className="attention-axis" key={`col-${name}`}>{name}</span>)}
                  {QOIN_FEATURE_NAMES.map((rowName, rowIndex) => (
                    <Fragment key={rowName}>
                      <span className="attention-axis attention-row-label">{QOIN_FEATURE_SHORT_NAMES[rowIndex]}</span>
                      {QOIN_FEATURE_NAMES.map((columnName, columnIndex) => {
                        const weight = vector.attention?.[rowIndex]?.[columnIndex] ?? 0;
                        return <span
                          className="attention-cell"
                          key={columnName}
                          title={`${rowName} → ${columnName}: ${weight.toFixed(3)}`}
                          style={{ background: `rgba(40, 103, 220, ${0.08 + weight * 0.82})`, color: weight > 0.55 ? "#FFFFFF" : "#2B4160" }}
                        >{weight.toFixed(2)}</span>;
                      })}
                    </Fragment>
                  ))}
                </div>
              </section>
              <section className="strategy-map-panel">
            <div className="attention-heading"><strong>Active strategy map</strong><span>{decisionStrategy?.version || "Strategy unavailable"} · {decisionStrategy?.status || "unknown status"}</span></div>
            {decisionStrategy ? <>
              <div className="strategy-gates">
                <div><span>Angular threshold</span><strong>{decisionStrategy.angular_threshold_degrees.toFixed(2)}°</strong></div>
                <div><span>Magnitude threshold</span><strong>{decisionStrategy.magnitude_threshold.toFixed(3)}</strong></div>
                <div><span>Maximum spread</span><strong>{decisionStrategy.max_spread_bps.toFixed(2)} bps</strong></div>
                <div><span>Attention scale</span><strong>{decisionStrategy.attention_scale.toFixed(3)}</strong></div>
              </div>
              <div className="strategy-weights">{QOIN_FEATURE_NAMES.map((name) => <div className="strategy-weight-row" key={name}>
                <strong>{name.replaceAll("_", " ")}</strong>
                <span>Bullish <b>{decisionStrategy.bullish_weights[name]?.toFixed(3) ?? "—"}</b></span>
                <span>Bearish <b>{decisionStrategy.bearish_weights[name]?.toFixed(3) ?? "—"}</b></span>
                <span>Scale <b>{decisionStrategy.feature_scales[name]?.toFixed(3) ?? "—"}</b></span>
              </div>)}</div>
              <p className="disclaimer">MongoDB-backed active strategy parameters. Last update: {new Date(decisionStrategy.updated_at_ms).toLocaleString()}.</p>
            </> : <p className="provider-empty">The active strategy map could not be loaded.</p>}
          </section>
          <section className="raw-tape">
            <button className="raw-tape-toggle" onClick={() => setShowRawEvents((value) => !value)}>Raw event tape · {rawEvents.length} observations <span>{showRawEvents ? "Hide" : "Show"}</span></button>
            {showRawEvents && <div className="raw-event-list">{rawEvents.slice(0,1000).map((event) => <details key={event.event_id}><summary><span>{event.event_type.toUpperCase()}</span><b>{event.price ?? event.funding_rate ?? "—"}</b><small>{new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</small></summary><pre>{JSON.stringify(event,null,2)}</pre></details>)}</div>}
          </section>
          <p className="disclaimer">{vector.reason || "Engine research output. It is not an order or a profitability claim."}</p>
            </>
          ) : <p className="disclaimer">{vectorError || "No persisted engine vector is available for this symbol yet."}</p>}
          <p className="disclaimer">The plotted arrow is the persisted research decision vector. Feature inputs and attention links are shown above. This is not an order or a profitability claim.</p>
          </> : <div className="mechanics-set-view">
            <SetLedgerPanel ledger={setLedger} status={setLedgerStatus} />
            <ResearchLedger decision={vector} history={vectorHistory} events={rawEvents} />
            <section className="raw-tape">
              <button className="raw-tape-toggle" onClick={() => setShowRawEvents((value) => !value)}>Persisted source-event tape · {rawEvents.length} observations <span>{showRawEvents ? "Hide" : "Show"}</span></button>
              {showRawEvents && <div className="raw-event-list">{rawEvents.slice(0,1000).map((event) => <details key={event.event_id}><summary><span>{event.event_type.toUpperCase()}</span><b>{event.price ?? event.funding_rate ?? "—"}</b><small>{new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</small></summary><pre>{JSON.stringify(event,null,2)}</pre></details>)}</div>}
            </section>
          </div>}
        </div>
        <footer className="modal-footer"><button className="secondary-button" onClick={closeInstrument}>Back to Archive</button><button className="primary-button" onClick={() => toggleArchive(selected.symbol)}>{selected.archived ? "Remove from archive" : "Add to archive"}</button></footer>
      </section></div>}
    </div>
  );
}
