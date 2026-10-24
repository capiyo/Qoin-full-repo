import { useEffect, useMemo, useState } from "react";
import {
  evaluateQoinSignal,
  getQoinHealth,
  type MarketForceObservation,
  type QoinHealth,
  type QoinSignal,
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

  useEffect(() => {
    getQoinHealth().then(setHealth).catch(() => setHealth(null));
  }, []);

  const activeForces = useMemo(
    () => fields.filter(({ key }) => Math.abs(forces[key]) > 0.0001).length,
    [forces],
  );

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
