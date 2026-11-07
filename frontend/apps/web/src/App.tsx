import { useEffect, useMemo, useState } from "react";
import {
  evaluateQoinSignal,
  getQoinHealth,
  getOperatingCycle,
  configureOperatingCycle,
  startOperatingCycle,
  stopOperatingCycle,
  runPaperBacktest,
  getPaperBacktestHistory,
  getPaperSession,
  getPaperSessionHistory,
  type PaperBacktestReport,
  type PaperBacktestHistoryItem,
  type MarketForceObservation,
  type QoinHealth,
  type QoinSignal,
  type OperatingCycleConfig,
  type OperatingCycleSnapshot,
  type PaperAccountConfig,
  type PaperSessionSnapshot,
  type PaperMirrorEvent,
  type PaperTradeHistoryItem,
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
  const [cycle, setCycle] = useState<OperatingCycleSnapshot | null>(null);
  const [cycleConfig, setCycleConfig] = useState<OperatingCycleConfig>({ observation_minutes: 5, weight_evaluation_minutes: 60, session_minutes: 480, mode: "paper", paper_accounts: [] });
  const [cycleBusy, setCycleBusy] = useState(false);
  const [cycleError, setCycleError] = useState("");
  const [paperSession, setPaperSession] = useState<PaperSessionSnapshot | null>(null);
  const [paperHistory, setPaperHistory] = useState<{ decisions: PaperMirrorEvent[]; trades: PaperTradeHistoryItem[] }>({ decisions: [], trades: [] });
  const [paperAccountDraft, setPaperAccountDraft] = useState<PaperAccountConfig>({ symbol: "", venue: "", quote_currency: "USD", initial_cash: 0, notional_per_trade: 0, reinvest_profits: true, fee_bps: 10, slippage_bps: 5 });
  const [backtest, setBacktest] = useState<PaperBacktestReport | null>(null);
  const [backtestHistory, setBacktestHistory] = useState<PaperBacktestHistoryItem[]>([]);
  const [backtestBusy, setBacktestBusy] = useState(false);
  const [backtestError, setBacktestError] = useState("");
  const [backtestForm, setBacktestForm] = useState({ symbol: "BTCUSDT", venue: "", quote_currency: "USDT", lookback_hours: 24, warmup_hours: 24, initial_cash: 0, notional_per_trade: 0, reinvest_profits: true, fee_bps: 10, slippage_bps: 5 });

  useEffect(() => {
    getQoinHealth().then(setHealth).catch(() => setHealth(null));
    getPaperBacktestHistory().then((result) => setBacktestHistory(result.runs)).catch(() => setBacktestHistory([]));
    getOperatingCycle().then((snapshot) => { setCycle(snapshot); setCycleConfig(snapshot.config); }).catch(() => setCycle(null));
    getPaperSession().then(setPaperSession).catch(() => setPaperSession(null));
    getPaperSessionHistory(undefined, 50).then(setPaperHistory).catch(() => setPaperHistory({ decisions: [], trades: [] }));
    const refreshCycle = () => { getOperatingCycle().then(setCycle).catch(() => setCycle(null)); getPaperSession().then(setPaperSession).catch(() => setPaperSession(null)); getPaperSessionHistory(undefined, 50).then(setPaperHistory).catch(() => undefined); };
    const timer = window.setInterval(refreshCycle, 5000);
    return () => window.clearInterval(timer);
  }, []);

  const activeForces = useMemo(
    () => fields.filter(({ key }) => Math.abs(forces[key]) > 0.0001).length,
    [forces],
  );

  async function saveCycle() {
    setCycleBusy(true); setCycleError("");
    try { const snapshot = await configureOperatingCycle(cycleConfig); setCycle(snapshot); }
    catch (e) { setCycleError(e instanceof Error ? e.message : "Could not save cycle configuration"); }
    finally { setCycleBusy(false); }
  }

  async function runCycle(action: "start" | "stop") {
    setCycleBusy(true); setCycleError("");
    try {
      if (action === "start") { await configureOperatingCycle(cycleConfig); setCycle(await startOperatingCycle()); }
      else setCycle(await stopOperatingCycle());
      setPaperSession(await getPaperSession());
      setPaperHistory(await getPaperSessionHistory(undefined, 50));
    }
    catch (e) { setCycleError(e instanceof Error ? e.message : "Could not update operating cycle"); }
    finally { setCycleBusy(false); }
  }

  function addPaperAccount() {
    const account = { ...paperAccountDraft, symbol: paperAccountDraft.symbol.trim().toUpperCase(), venue: paperAccountDraft.venue.trim(), quote_currency: paperAccountDraft.quote_currency.trim().toUpperCase() };
    if (!account.symbol || !account.venue || !account.quote_currency || account.initial_cash <= 0 || account.notional_per_trade <= 0 || account.notional_per_trade > account.initial_cash) {
      setCycleError("Enter an exact symbol and venue, quote currency, positive starting capital, and a notional no greater than that capital."); return;
    }
    if (cycleConfig.paper_accounts.some((item) => item.symbol === account.symbol && item.venue === account.venue)) { setCycleError("That venue and symbol are already configured."); return; }
    if (cycleConfig.paper_accounts.length >= 50) { setCycleError("A cycle supports at most 50 instruments."); return; }
    setCycleConfig((current) => ({ ...current, paper_accounts: [...current.paper_accounts, account] }));
    setCycleError("");
  }

  function removePaperAccount(symbol: string, venue: string) {
    setCycleConfig((current) => ({ ...current, paper_accounts: current.paper_accounts.filter((item) => item.symbol !== symbol || item.venue !== venue) }));
  }

  function formatRemaining(seconds: number) {
    const hours = Math.floor(seconds / 3600); const minutes = Math.floor((seconds % 3600) / 60); const secs = seconds % 60;
    return hours ? `${hours}h ${minutes}m` : `${minutes}m ${secs}s`;
  }

  async function runBacktest() {
    setBacktestBusy(true); setBacktestError(""); setBacktest(null);
    const end = Date.now();
    try {
      const result = await runPaperBacktest({ symbol: backtestForm.symbol.trim().toUpperCase(), venue: backtestForm.venue.trim(), quote_currency: backtestForm.quote_currency.trim().toUpperCase(), start_ts_ms: end - backtestForm.lookback_hours * 3600000, end_ts_ms: end, warmup_hours: backtestForm.warmup_hours, initial_cash: backtestForm.initial_cash, notional_per_trade: backtestForm.notional_per_trade, reinvest_profits: backtestForm.reinvest_profits, fee_bps: backtestForm.fee_bps, slippage_bps: backtestForm.slippage_bps, max_events: 100000 });
      setBacktest(result);
      const history = await getPaperBacktestHistory(backtestForm.symbol.trim().toUpperCase());
      setBacktestHistory(history.runs);
    } catch (e) { setBacktestError(e instanceof Error ? e.message : "Paper replay failed"); }
    finally { setBacktestBusy(false); }
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
      <section className="panel cycle-panel">
        <div className="panel-heading"><div><p className="eyebrow">OPERATING POLICY</p><h2>Observation → validation → session</h2></div><span className="counter">{cycle?.phase?.replaceAll("_", " ").toUpperCase() ?? "LOADING"}</span></div>
        <p className="hint">Five-minute initial observation, one-hour signal/weight diagnostics, then a configurable paper-monitoring session. Existing coefficients remain locked; diagnostics do not silently train on recent outcomes.</p>
        <div className="cycle-grid">
          <label className="cycle-field"><span>Observation (minutes)</span><input type="number" min="1" max="120" value={cycleConfig.observation_minutes} disabled={!!cycle?.active} onChange={(e) => setCycleConfig((v) => ({ ...v, observation_minutes: Number(e.target.value) }))} /></label>
          <label className="cycle-field"><span>Weight evaluation (minutes)</span><input type="number" min="1" max="720" value={cycleConfig.weight_evaluation_minutes} disabled={!!cycle?.active} onChange={(e) => setCycleConfig((v) => ({ ...v, weight_evaluation_minutes: Number(e.target.value) }))} /></label>
          <label className="cycle-field"><span>Paper session (minutes)</span><input type="number" min="1" max="1440" value={cycleConfig.session_minutes} disabled={!!cycle?.active} onChange={(e) => setCycleConfig((v) => ({ ...v, session_minutes: Number(e.target.value) }))} /></label>
        </div>
        <div className="cycle-diagnostics">
          <strong>Paper accounts · {cycleConfig.paper_accounts.length}/50</strong>
          <p className="hint">Each venue/symbol has its own isolated quote-currency ledger. Add one account per instrument; capital is not converted between KSh, USD, USDT or other currencies.</p>
          <div className="cycle-grid">
            <label className="cycle-field"><span>Symbol</span><input value={paperAccountDraft.symbol} onChange={(e) => setPaperAccountDraft(v => ({...v, symbol:e.target.value}))} placeholder="BTCUSDT" /></label>
            <label className="cycle-field"><span>Exact venue ID</span><input value={paperAccountDraft.venue} onChange={(e) => setPaperAccountDraft(v => ({...v, venue:e.target.value}))} placeholder="Must match ingestion venue" /></label>
            <label className="cycle-field"><span>Quote currency</span><input value={paperAccountDraft.quote_currency} onChange={(e) => setPaperAccountDraft(v => ({...v, quote_currency:e.target.value}))} placeholder="USDT / USD / KES" /></label>
            <label className="cycle-field"><span>Starting capital</span><input type="number" min="0" value={paperAccountDraft.initial_cash} onChange={(e) => setPaperAccountDraft(v => ({...v, initial_cash:Number(e.target.value)}))} /></label>
            <label className="cycle-field"><span>Notional per entry</span><input type="number" min="0" value={paperAccountDraft.notional_per_trade} onChange={(e) => setPaperAccountDraft(v => ({...v, notional_per_trade:Number(e.target.value)}))} /></label>
            <label className="cycle-field"><span>Fee (bps per side)</span><input type="number" min="0" max="100" value={paperAccountDraft.fee_bps} onChange={(e) => setPaperAccountDraft(v => ({...v, fee_bps:Number(e.target.value)}))} /></label>
            <label className="cycle-field"><span>Slippage (bps per side)</span><input type="number" min="0" max="100" value={paperAccountDraft.slippage_bps} onChange={(e) => setPaperAccountDraft(v => ({...v, slippage_bps:Number(e.target.value)}))} /></label>
            <label className="cycle-check"><input type="checkbox" checked={paperAccountDraft.reinvest_profits} onChange={(e) => setPaperAccountDraft(v => ({...v, reinvest_profits:e.target.checked}))} /><span>Reinvest profits / compound notional</span></label>
          </div>
          <button className="secondary" onClick={addPaperAccount} disabled={!!cycle?.active}>Add instrument account</button>
          {cycleConfig.paper_accounts.map((account) => <div className="cycle-meta" key={`${account.venue}:${account.symbol}`}><span><strong>{account.symbol}</strong> · {account.venue} · {account.quote_currency} {account.initial_cash.toLocaleString()} · entry {account.notional_per_trade.toLocaleString()}</span><button className="secondary" onClick={() => removePaperAccount(account.symbol, account.venue)} disabled={!!cycle?.active}>Remove</button></div>)}
        </div>
        <div className="cycle-meta"><span>Time remaining: <strong>{cycle ? formatRemaining(cycle.phase_seconds_remaining) : "—"}</strong></span><span>Observations: <strong>{cycle?.observations ?? 0}</strong></span><span>Evaluations: <strong>{cycle?.signal_evaluations ?? 0}</strong></span><span>Mode: <strong>{cycle?.config.mode ?? "paper"}</strong></span><span>Mean |score|: <strong>{cycle?.mean_absolute_score?.toFixed(3) ?? "—"}</strong></span><span>Mean alignment: <strong>{cycle?.mean_alignment?.toFixed(3) ?? "—"}</strong></span><span>BUY / SELL / WAIT: <strong>{cycle?.buy_signals ?? 0} / {cycle?.sell_signals ?? 0} / {cycle?.wait_signals ?? 0}</strong></span></div>
        {cycle && Object.keys(cycle.mean_feature_values).length > 0 && <div className="cycle-diagnostics">
          <strong>Mean feature activations</strong>
          <div>{Object.entries(cycle.mean_feature_values).map(([name, value]) => <span key={name}>{name}: <b>{value.toFixed(3)}</b></span>)}</div>
        </div>}
        {cycle && cycle.mean_attention.length > 0 && <div className="cycle-diagnostics">
          <strong>Mean attention matrix</strong>
          {cycle.mean_attention.map((row, index) => <span key={index}>Row {index + 1}: {row.map((value) => value.toFixed(3)).join(" · ")}</span>)}
        </div>}
        <div className="cycle-actions"><button className="secondary" onClick={saveCycle} disabled={cycleBusy || !!cycle?.active}>Save schedule</button><button className="primary cycle-start" onClick={() => runCycle("start")} disabled={cycleBusy || !!cycle?.active || (cycleConfig.mode === "paper" && cycleConfig.paper_accounts.length === 0)}>Start cycle</button><button className="secondary" onClick={() => runCycle("stop")} disabled={cycleBusy || !cycle?.active}>Stop cycle</button></div>
        {cycleError && <p className="error" role="alert">{cycleError}</p>}
        <p className="cycle-safety">Paper phase: forward event-driven fills are simulated only for configured venue/symbol accounts · no broker orders · live execution disabled.</p>
      </section>
      <section className="panel cycle-panel">
        <div className="panel-heading"><div><p className="eyebrow">MIRROR / FORWARD PAPER SESSION</p><h2>Paper account ledger</h2></div><span className="counter">{paperSession?.active ? "SESSION ACTIVE" : "SESSION INACTIVE"}</span></div>
        <p className="hint">Fills use the next valid observed two-sided quote, with configured fees and slippage. Each account is isolated by venue and quote currency.</p>
        {paperSession?.accounts.length ? paperSession.accounts.map((account) => <div className="cycle-diagnostics" key={`${account.venue}:${account.symbol}`}><strong>{account.symbol} · {account.venue} · {account.quote_currency}</strong><div className="cycle-meta"><span>Equity <strong>{account.equity.toFixed(2)}</strong></span><span>Net P/L <strong>{account.net_pnl.toFixed(2)}</strong></span><span>Realized <strong>{account.realized_pnl.toFixed(2)}</strong></span><span>Unrealized <strong>{account.unrealized_pnl.toFixed(2)}</strong></span><span>Fees <strong>{account.fees_paid.toFixed(2)}</strong></span><span>Max DD <strong>{account.max_drawdown_pct.toFixed(2)}%</strong></span><span>Entries / exits <strong>{account.entries} / {account.exits}</strong></span><span>Events <strong>{account.events}</strong></span><span>Position <strong>{account.has_open_position ? `OPEN · ${account.open_position?.quantity.toFixed(6)} @ ${account.open_position?.entry_price.toFixed(5)}` : "FLAT"}</strong></span></div></div>) : <p className="hint">No paper account activity yet. Configure accounts above, then start a paper cycle.</p>}
        <p className="cycle-safety">SIMULATED EXECUTION ONLY · LIVE BROKER ROUTING DISABLED</p>
        <div className="cycle-diagnostics"><strong>Recent Mirror decisions</strong>
          {paperHistory.decisions.slice(0, 8).map((item) => <div className="cycle-meta" key={`${item.venue}:${item.symbol}:${item.event_id}`}><span>{item.symbol} · {item.venue} · {new Date(item.ts_ms).toLocaleTimeString()}</span><span><strong>{item.action.toUpperCase()}</strong> · {item.execution}</span><span>{item.reason}</span><span>Equity {item.equity.toFixed(2)} {item.quote_currency}</span></div>)}
          {!paperHistory.decisions.length && <p className="hint">Mirror decisions appear when configured instruments send events during the paper-session phase.</p>}
        </div>
        <div className="cycle-diagnostics"><strong>Closed paper trades</strong>
          {paperHistory.trades.slice(0, 8).map((trade, index) => <div className="cycle-meta" key={`${trade.cycle_id}:${trade.venue}:${trade.symbol}:${trade.exit_ts_ms}:${index}`}><span>{trade.symbol} · {trade.venue} · {new Date(trade.exit_ts_ms).toLocaleString()}</span><span>Net P/L <strong>{trade.net_pnl.toFixed(2)} {trade.quote_currency}</strong></span><span>Return {trade.return_pct.toFixed(3)}% · Fees {trade.fees.toFixed(2)}</span></div>)}
          {!paperHistory.trades.length && <p className="hint">No closed paper trades yet.</p>}
        </div>
      </section>
      <section className="panel backtest-panel">
        <div className="panel-heading"><div><p className="eyebrow">MIRROR / CHRONOLOGICAL REPLAY</p><h2>Paper backtest</h2></div><span className="counter">NO LIVE ORDERS</span></div>
        <p className="hint">Replays persisted MongoDB market events in event-time order using the active EdgeStrategy. Entries fill at observed ask, exits at observed bid, with your fee and slippage assumptions. Long-only; one position at a time. Capital and notional use the instrument quote currency: convert KSh 20,000 to USDT first for BTCUSDT; no FX conversion is applied.</p>
        <div className="cycle-grid">
          <label className="cycle-field"><span>Instrument symbol</span><input value={backtestForm.symbol} onChange={(e) => setBacktestForm(v => ({...v, symbol:e.target.value}))} /></label>
          <label className="cycle-field"><span>Exact venue ID</span><input value={backtestForm.venue} placeholder="Must match stored event venue" onChange={(e) => setBacktestForm(v => ({...v, venue:e.target.value}))} /></label>
          <label className="cycle-field"><span>Quote currency (capital unit)</span><input value={backtestForm.quote_currency} onChange={(e) => setBacktestForm(v => ({...v, quote_currency:e.target.value}))} /></label>
          <label className="cycle-field"><span>Lookback (hours)</span><input type="number" min="1" max="8760" value={backtestForm.lookback_hours} onChange={(e) => setBacktestForm(v => ({...v, lookback_hours:Math.max(1,Math.min(8760,Number(e.target.value)||1))}))} /></label>
          <label className="cycle-field"><span>Warmup history (hours)</span><input type="number" min="1" max="168" value={backtestForm.warmup_hours} onChange={(e) => setBacktestForm(v => ({...v, warmup_hours:Math.max(1,Math.min(168,Number(e.target.value)||1))}))} /></label>
          <label className="cycle-field"><span>Starting capital ({backtestForm.quote_currency.toUpperCase()})</span><input type="number" min="1" value={backtestForm.initial_cash} onChange={(e) => setBacktestForm(v => ({...v, initial_cash:Math.max(1,Number(e.target.value)||1)}))} /></label>
          <label className="cycle-field"><span>Notional per entry ({backtestForm.quote_currency.toUpperCase()})</span><input type="number" min="1" value={backtestForm.notional_per_trade} onChange={(e) => setBacktestForm(v => ({...v, notional_per_trade:Math.max(1,Number(e.target.value)||1)}))} /></label>
          <label className="cycle-check"><input type="checkbox" checked={backtestForm.reinvest_profits} onChange={(e) => setBacktestForm(v => ({...v, reinvest_profits:e.target.checked}))} /><span>Reinvest profits and compound future entry notional</span></label>
          <label className="cycle-field"><span>Fees (basis points / side)</span><input type="number" min="0" max="100" step="0.5" value={backtestForm.fee_bps} onChange={(e) => setBacktestForm(v => ({...v, fee_bps:Math.max(0,Math.min(100,Number(e.target.value)||0))}))} /></label>
          <label className="cycle-field"><span>Slippage (basis points)</span><input type="number" min="0" max="100" step="0.5" value={backtestForm.slippage_bps} onChange={(e) => setBacktestForm(v => ({...v, slippage_bps:Math.max(0,Math.min(100,Number(e.target.value)||0))}))} /></label>
        </div>
        <button className="primary backtest-run" onClick={runBacktest} disabled={backtestBusy || !backtestForm.symbol.trim() || !backtestForm.venue.trim() || !backtestForm.quote_currency.trim() || backtestForm.notional_per_trade > backtestForm.initial_cash}>{backtestBusy ? "Replaying event history…" : "Run chronological paper replay"}</button>
        {backtestError && <p className="error" role="alert">{backtestError}</p>}
        {backtest && <>
          <div className="backtest-results">
            <div><small>Net P&amp;L</small><strong>{backtest.net_pnl.toFixed(2)}</strong></div>
            <div><small>Return</small><strong>{backtest.return_pct.toFixed(2)}%</strong></div>
            <div><small>Max drawdown</small><strong>{backtest.max_drawdown_pct.toFixed(2)}%</strong></div>
            <div><small>Win rate</small><strong>{backtest.win_rate_pct == null ? "—" : `${backtest.win_rate_pct.toFixed(1)}%`}</strong></div>
            <div><small>Closed trades</small><strong>{backtest.executed_exits}</strong></div>
            <div><small>Fees paid</small><strong>{backtest.fees_paid.toFixed(2)}</strong></div>
            <div><small>Events / signals</small><strong>{backtest.event_count} / {backtest.signal_count}</strong></div>
            <div><small>Warmup events</small><strong>{backtest.warmup_event_count}</strong></div>
            <div><small>Entries skipped</small><strong>{backtest.skipped_entries}</strong></div>
            <div><small>Pending intent</small><strong>{backtest.pending_order ?? "—"}</strong></div>
          </div>
          <div className="cycle-meta"><span>Run: <strong>{backtest.run_id}</strong></span><span>Venue: <strong>{backtest.venue}</strong></span><span>Capital unit: <strong>{backtest.quote_currency}</strong></span><span>Compounding: <strong>{backtest.reinvest_profits ? "ON" : "OFF"}</strong></span><span>Strategy: <strong>{backtest.strategy_version}</strong></span><span>Mirror decisions saved: <strong>{backtest.decisions.length}</strong></span></div>
          <p className="hint">{backtest.assumptions}</p>
          {backtest.event_count === 0 && <p className="error">No persisted events found for this symbol and time window. This is not a zero-return strategy test; ingest historical market data first.</p>}
          {backtest.open_position && <p className="hint">Open position remains marked, not force-closed: quantity {backtest.open_position.quantity.toFixed(6)} · entry {backtest.open_position.entry_price.toFixed(6)}.</p>}
          <details className="mirror-trace">
            <summary>Mirror decision trace · latest 12 of {backtest.decisions.length}</summary>
            <div className="mirror-list">
              {backtest.decisions.slice(-12).reverse().map((item) => (
                <article className="mirror-item" key={item.sequence + "-" + item.event_id}>
                  <div><strong>{item.action.toUpperCase()}</strong><small>{new Date(item.ts_ms).toLocaleString()}</small></div>
                  <span>{item.execution.replaceAll("_", " ")}</span>
                  <small>{item.reason}</small>
                  <small>Vector [{item.decision_vector.map((value) => value.toFixed(3)).join(", ")}] · magnitude {item.magnitude.toFixed(3)} · spread {item.spread_bps == null ? "—" : item.spread_bps.toFixed(2) + " bps"}</small>
                  <small>Forces: {Object.entries(item.features).map(([name, value]) => name + " " + value.toFixed(2)).join(" · ")}</small>
                </article>
              ))}
            </div>
          </details>
          <details className="mirror-trace">
            <summary>Simulated trade ledger · {backtest.trades.length} closed</summary>
            <div className="mirror-list">
              {backtest.trades.slice(-12).reverse().map((trade, index) => (
                <article className="mirror-item" key={trade.entry_ts_ms + "-" + index}>
                  <div><strong>NET {trade.net_pnl?.toFixed(2) ?? "—"}</strong><small>{trade.return_pct?.toFixed(2) ?? "—"}%</small></div>
                  <span>{trade.exit_reason ?? "closed"}</span>
                  <small>Entry {new Date(trade.entry_ts_ms).toLocaleString()} · Exit {trade.exit_ts_ms ? new Date(trade.exit_ts_ms).toLocaleString() : "open"}</small>
                </article>
              ))}
            </div>
          </details>
        </>}
        {backtestHistory.length > 0 && <div className="backtest-history">
          <h3>Saved Mirror runs</h3>
          {backtestHistory.slice(0, 8).map((run) => (
            <div className="history-row" key={run.run_id}>
              <span><strong>{run.symbol}</strong><small>{run.run_id}</small></span>
              <span><strong>{run.net_pnl.toFixed(2)}</strong><small>{run.return_pct.toFixed(2)}% · DD {run.max_drawdown_pct.toFixed(2)}%</small></span>
            </div>
          ))}
        </div>}
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
