import { useEffect, useMemo, useState } from "react";
import {
  evaluateQoinSignal,
  getQoinHealth,
  getQoinInstruments,
  getQoinMarketHorizons,
  type MarketForceObservation,
  type QoinHealth,
  type QoinAssetClass,
  type QoinSignal,
  type QoinMarketHorizonsResponse,
} from "@qoin/core";

const initial: MarketForceObservation = {
  ofi: 1.2,
  liquidity: 0.4,
  volatility: 0.3,
  funding: -0.2,
  open_interest_change: 0.6,
  liquidation_pressure: 0.1,
  basis: 0,
};

const fields: { key: keyof MarketForceObservation; label: string; detail: string }[] = [
  { key: "ofi", label: "Order-flow imbalance", detail: "Directional pressure from aggressive order flow" },
  { key: "liquidity", label: "Liquidity / depth", detail: "Normalized depth and liquidity change" },
  { key: "volatility", label: "Volatility", detail: "Context and signal dampening" },
  { key: "funding", label: "Funding", detail: "Crowding and carry pressure" },
  { key: "open_interest_change", label: "Open interest change", detail: "Change in open positions" },
  { key: "liquidation_pressure", label: "Liquidation pressure", detail: "Forced-flow intensity" },
  { key: "basis", label: "Basis", detail: "Perpetual / spot dislocation" },
];

export default function App() {
  const [forces, setForces] = useState(initial);
  const [health, setHealth] = useState<QoinHealth | null>(null);
  const [signal, setSignal] = useState<QoinSignal | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [symbol, setSymbol] = useState("BTCUSDT");
  const [assetClass, setAssetClass] = useState<QoinAssetClass>("crypto");
  const [availableSymbols, setAvailableSymbols] = useState<string[]>([]);
  const [marketHorizons, setMarketHorizons] = useState<QoinMarketHorizonsResponse | null>(null);
  const [marketBusy, setMarketBusy] = useState(false);
  const [marketError, setMarketError] = useState("");

  useEffect(() => {
    getQoinHealth().then(setHealth).catch(() => setHealth(null));
  }, []);

  useEffect(() => {
    getQoinInstruments(assetClass)
      .then((catalogue) => setAvailableSymbols(catalogue.symbols))
      .catch(() => setAvailableSymbols([]));
  }, [assetClass]);

  const activeForces = useMemo(
    () => fields.filter(({ key }) => Math.abs(forces[key]) > 0.0001).length,
    [forces],
  );

  async function loadMarketHorizons() {
    setMarketBusy(true);
    setMarketError("");
    try {
      setMarketHorizons(await getQoinMarketHorizons(symbol.trim().toUpperCase(), assetClass));
    } catch (e) {
      setMarketError(e instanceof Error ? e.message : "Unable to load market observations");
      setMarketHorizons(null);
    } finally {
      setMarketBusy(false);
    }
  }

  async function evaluate() {
    setBusy(true);
    setError("");
    try {
      setSignal(await evaluateQoinSignal({
        observation: forces,
        has_open_position: false,
      }));
    } catch (e) {
      setError(e instanceof Error ? e.message : "Unable to reach Qoin API");
    } finally {
      setBusy(false);
    }
  }

  return (
    <main className="shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Qoin home"><span className="brand-mark">Q</span> QOIN</a>
        <div className="status"><span className={health ? "dot online" : "dot"} />{health ? "API connected · research mode" : "API offline"}</div>
      </header>
      <section className="intro">
        <p className="eyebrow">MARKET-FORCE ENGINE / RESEARCH</p>
        <h1>Read the forces.<br /><span>Respect uncertainty.</span></h1>
        <p className="lede">A present-centered view of market pressure. Explore normalized inputs and request a deterministic research signal from the Rust engine.</p>
      </section>
      <section className="panel market-panel">
        <div className="panel-heading"><div><p className="eyebrow">STORED MARKET DATA</p><h2>Multi-horizon research</h2></div><span className="counter">1m → 1w</span></div>
        <p className="hint">Builds separate trailing observations from ingested trade, order-book, funding, open-interest and liquidation events. {availableSymbols.length} {assetClass} instruments currently have ingested events.</p>
        <div className="market-controls">
          <label>Asset class <select aria-label="Asset class" value={assetClass} onChange={(event) => setAssetClass(event.target.value as QoinAssetClass)}><option value="forex">Forex</option><option value="crypto">Crypto</option><option value="stock">Stocks</option></select></label>
          <label>Instrument <input aria-label="Instrument symbol" list="qoin-symbols" value={symbol} onChange={(event) => setSymbol(event.target.value)} maxLength={64} /><datalist id="qoin-symbols">{availableSymbols.map((item) => <option key={item} value={item} />)}</datalist></label>
          <button className="secondary" onClick={loadMarketHorizons} disabled={marketBusy || !symbol.trim()}>{marketBusy ? "Loading…" : "Load market horizons"}</button>
        </div>
        {marketError && <p className="error" role="alert">{marketError}</p>}
        {marketHorizons ? (
          <>
            <div className="horizon-grid">
              {marketHorizons.observations.map((item) => {
                const signal = marketHorizons.signals?.horizons.find((entry) => entry.horizon === item.horizon)?.signal;
                return <div className="horizon-card" key={item.horizon}>
                  <div className="horizon-title"><strong>{item.horizon.replace("_", " ")}</strong><span className={item.ready ? "ready" : "not-ready"}>{item.ready ? "READY" : "WAITING FOR DATA"}</span></div>
                  <div className="horizon-signal">{signal?.decision ?? "—"}</div>
                  <div className="horizon-meta">{item.event_count} events · {item.trade_count} trades · {item.book_count} books</div>
                  <div className="horizon-meta">Window coverage {(item.coverage_ratio * 100).toFixed(1)}%</div>
                  <div className="horizon-meta">OFI {item.observation.ofi.toFixed(2)} · OI Δ {item.observation.open_interest_change.toFixed(3)}</div>
                  {!item.ready && <p className="horizon-reason">{item.reasons.slice(0, 2).join("; ")}</p>}
                </div>;
              })}
            </div>
            <p className="hint">Loaded {marketHorizons.events_loaded} {marketHorizons.asset_class ?? assetClass} events for {marketHorizons.symbol}. {marketHorizons.event_limit_reached ? "The 50,000-event retrieval cap was reached; older records may be omitted." : "Only events at or before the decision timestamp were considered."}</p>
            {marketHorizons.signals && <p className="consensus-line">Horizon relationship: <strong>{marketHorizons.signals.consensus.relationship.replaceAll("_", " ")}</strong> · BUY {marketHorizons.signals.consensus.buy_count} · SELL {marketHorizons.signals.consensus.sell_count}</p>}
          </>
        ) : <p className="empty-market">No market-backed horizon result loaded. Ingest real venue events first; the evaluator will abstain where trade or book evidence is missing.</p>}
        <div className="safety-note"><strong>Research only</strong><p>These are descriptive signals, not orders, position sizing or a profitability claim.</p></div>
      </section>
      <div className="layout">
        <section className="panel forces-panel">
          <div className="panel-heading"><div><p className="eyebrow">INPUT VECTOR</p><h2>Market forces</h2></div><span className="counter">{activeForces} / 7 active</span></div>
          <p className="hint">Use normalized values. This prototype does not fetch exchange data.</p>
          <div className="force-list">
            {fields.map(({ key, label, detail }) => (
              <label className="force-row" key={key}>
                <span className="force-copy"><strong>{label}</strong><small>{detail}</small></span>
                <input
                  aria-label={label}
                  type="number"
                  step="0.1"
                  value={forces[key]}
                  onChange={(event) => setForces((prev) => ({ ...prev, [key]: Number(event.target.value) }))}
                />
              </label>
            ))}
          </div>
          <button className="primary" onClick={evaluate} disabled={busy}>{busy ? "Evaluating…" : "Evaluate research signal"}</button>
          {error && <p className="error" role="alert">{error}</p>}
        </section>
        <aside className="panel signal-panel">
          <p className="eyebrow">VECTOR CONVERGENCE</p>
          <h2>Signal output</h2>
          {signal ? (
            <>
              <div className={`signal-badge ${signal.decision.toLowerCase()}`}>{signal.decision}</div>
              <div className="score-line"><span>Directional score</span><strong>{signal.score.toFixed(3)}</strong></div>
              <div className="meter"><span style={{ width: `${Math.min(100, Math.abs(signal.score) * 100)}%` }} /></div>
              <div className="score-line"><span>Angular alignment</span><strong>{signal.alignment.toFixed(3)}</strong></div>
              <div className="score-line"><span>Angle (radians)</span><strong>{signal.angle_radians.toFixed(3)}</strong></div>
              <div className="score-line"><span>Active forces</span><strong>{signal.active_force_count} / 7</strong></div>
              <div className="score-line"><span>Configuration</span><strong>{signal.config_version}</strong></div>
              <p className="hint">{signal.rationale}</p>
            </>
          ) : <div className="empty"><div className="empty-orbit">↗</div><strong>Waiting for evaluation</strong><p>Adjust forces, then ask the engine for a signal.</p></div>}
          <div className="safety-note"><strong>Research only</strong><p>Signals are not orders. This UI cannot place trades. Live trading is disabled and profitability has not been established.</p></div>
        </aside>
      </div>
      <footer>QOIN / EXPERIMENTAL MARKET-FLOW SYSTEM <span>NO LIVE EXECUTION</span></footer>
    </main>
  );
}
