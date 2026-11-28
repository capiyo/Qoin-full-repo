import { useEffect, useState } from "react";
import { getQoinMarketHorizons, type QoinMarketHorizonsResponse } from "@qoin/core";

type InstrumentOption = {
  symbol: string;
  name: string;
  market: string;
  engineSymbol: string;
  venue: string;
  engineVenue: string;
};

type Props = { instruments: InstrumentOption[] };

function instrumentKey(item: InstrumentOption) {
  return item.engineSymbol + "::" + item.engineVenue;
}

export function MultiHorizonPanel({ instruments }: Props) {
  const available = instruments.filter((item) => item.engineSymbol.trim() && item.engineVenue.trim());
  const [selectedKey, setSelectedKey] = useState("");
  const [minEvents, setMinEvents] = useState("20");
  const [result, setResult] = useState<QoinMarketHorizonsResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!available.length) {
      setSelectedKey("");
      setResult(null);
      return;
    }
    if (!available.some((item) => instrumentKey(item) === selectedKey)) {
      setSelectedKey(instrumentKey(available[0]));
      setResult(null);
    }
  }, [instruments, selectedKey]);

  const selected = available.find((item) => instrumentKey(item) === selectedKey);
  const load = async () => {
    if (!selected) return;
    const minimum = Number(minEvents);
    if (!Number.isInteger(minimum) || minimum < 5 || minimum > 10_000) {
      setError("Minimum events must be between 5 and 10,000.");
      return;
    }
    setBusy(true);
    setError("");
    try {
      setResult(await getQoinMarketHorizons(selected.engineSymbol, selected.engineVenue, minimum));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Multi-horizon research is unavailable.");
      setResult(null);
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="algorithm-catalogue multi-horizon-panel">
      <div className="panel-heading">
        <div>
          <p className="eyebrow">CAUSAL RESEARCH · NO ORDER AUTHORITY</p>
          <h3>Multi-horizon research</h3>
          <p>Evaluate independent 1m to 1w windows for one exact venue and instrument</p>
        </div>
        <span className="counter">1m → 1w</span>
      </div>
      <div className="horizon-controls">
        <label>
          Instrument
          <select value={selectedKey} onChange={(event) => { setSelectedKey(event.target.value); setResult(null); }} disabled={!available.length}>
            {available.map((item) => (
              <option key={instrumentKey(item)} value={instrumentKey(item)}>
                {item.name} · {item.symbol} · {item.venue}
              </option>
            ))}
          </select>
        </label>
        <label>
          Minimum events per horizon
          <input type="number" min="5" max="10000" step="1" value={minEvents} onChange={(event) => setMinEvents(event.target.value)} />
        </label>
        <button className="primary-button" onClick={load} disabled={busy || !selected}>
          {busy ? "Evaluating…" : "Load horizons"}
        </button>
      </div>
      {error && <p className="horizon-error">{error}</p>}
      {!available.length && <p className="empty-market">The canonical instrument registry is unavailable. No fallback instruments are substituted.</p>}
      {result && (
        <>
          <div className="horizon-summary">
            <span>{result.symbol} · {result.venue_id}</span>
            <span>{result.events_loaded.toLocaleString()} events loaded</span>
            <span>{result.mode.replaceAll("_", " ")}</span>
          </div>
          <div className="horizon-grid">
            {result.observations.map((item) => {
              const signal = result.signals?.horizons.find((entry) => entry.horizon === item.horizon)?.signal;
              return (
                <article className="horizon-card" key={item.horizon}>
                  <div className="horizon-title">
                    <strong>{item.horizon.replaceAll("_", " ")}</strong>
                    <span className={item.ready ? "ready" : "not-ready"}>{item.ready ? "READY" : "WAITING"}</span>
                  </div>
                  <div className="horizon-signal">{signal?.decision ?? "—"}</div>
                  <div className="horizon-meta">{item.event_count} events · {item.trade_count} trades · {item.book_count} books</div>
                  <div className="horizon-meta">Coverage {(item.coverage_ratio * 100).toFixed(1)}% · Quality {(item.data_quality * 100).toFixed(0)}%</div>
                  <div className="horizon-meta">OFI {item.observation.ofi.toFixed(2)} · OI Δ {item.observation.open_interest_change.toFixed(3)}</div>
                  {!item.ready && <p className="horizon-reason">{item.reasons.slice(0, 2).join("; ")}</p>}
                  {signal && <p className="horizon-reason">Score {signal.score.toFixed(3)} · Alignment {signal.alignment.toFixed(3)}</p>}
                </article>
              );
            })}
          </div>
          {result.signals && (
            <p className="consensus-line">
              Horizon relationship: <strong>{result.signals.consensus.relationship.replaceAll("_", " ")}</strong>
              {" "}· BUY {result.signals.consensus.buy_count} · SELL {result.signals.consensus.sell_count}
              {" "}· WAIT {result.signals.consensus.wait_count}
            </p>
          )}
          {(result.event_limit_reached || result.invalid_event_documents > 0) && (
            <p className="horizon-error">
              Signals suppressed: {result.event_limit_reached ? "50,000-event safety cap reached. " : ""}
              {result.invalid_event_documents ? `${result.invalid_event_documents} event documents were invalid.` : ""}
            </p>
          )}
          <p className="hint">{result.note}</p>
        </>
      )}
      {!result && !busy && <p className="empty-market">Load stored market events to evaluate the eight independent horizons. Insufficient trade or book evidence produces WAITING, not a fabricated signal.</p>}
    </section>
  );
}
