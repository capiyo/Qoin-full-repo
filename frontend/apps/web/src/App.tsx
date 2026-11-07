import { useEffect, useMemo, useState } from "react";
import {
  getQoinHealth, getOperatingCycle, getPaperSession, getPaperSessionHistory, getPaperSessionRuns,
  type OperatingCycleSnapshot, type PaperSessionSnapshot, type PaperMirrorEvent, type PaperTradeHistoryItem, type PaperSessionRun,
} from "@qoin/core";

type Tab = "archive" | "explore";
type Instrument = { symbol: string; venue: string; currency: string; initialCash?: number; equity?: number; pnl?: number; status: string };
const discovery = [
  { symbol: "EURUSD", name: "Euro / US Dollar", category: "Forex", venue: "Discover" },
  { symbol: "GBPUSD", name: "British Pound / US Dollar", category: "Forex", venue: "Discover" },
  { symbol: "USDJPY", name: "US Dollar / Japanese Yen", category: "Forex", venue: "Discover" },
  { symbol: "BTCUSDT", name: "Bitcoin", category: "Crypto", venue: "Discover" },
  { symbol: "ETHUSDT", name: "Ethereum", category: "Crypto", venue: "Discover" },
  { symbol: "XAUUSD", name: "Gold", category: "Commodities", venue: "Discover" },
  { symbol: "XAGUSD", name: "Silver", category: "Commodities", venue: "Discover" },
  { symbol: "NVDA", name: "NVIDIA", category: "Stocks", venue: "Discover" },
  { symbol: "AAPL", name: "Apple", category: "Stocks", venue: "Discover" },
];

function VectorSpace({ event }: { event?: PaperMirrorEvent }) {
  const values = Object.entries(event?.features ?? {}).slice(0, 8);
  const points = values.map(([name, value], index) => {
    const angle = (Math.PI * 2 * index) / Math.max(values.length, 1) - Math.PI / 2;
    const radius = 34 + Math.min(1, Math.abs(value)) * 62;
    return { name, value, x: 160 + Math.cos(angle) * radius, y: 125 + Math.sin(angle) * radius };
  });
  return <div className="vector-canvas">
    <svg viewBox="0 0 320 250" role="img" aria-label="Instrument feature-space projection">
      {[34, 68, 102].map((r) => <circle key={r} cx="160" cy="125" r={r} className="vector-ring" />)}
      <path d="M20 125H300M160 12V238" className="vector-axis" />
      {points.map((p) => <g key={p.name}><line x1="160" y1="125" x2={p.x} y2={p.y} className="vector-spoke" /><circle cx={p.x} cy={p.y} r="5" className="vector-point" /><text x={p.x} y={p.y - 10} textAnchor="middle" className="vector-label">{p.name.replaceAll("_", " ")}</text></g>)}
      <circle cx="160" cy="125" r="8" className="vector-origin" />
    </svg>
    <div className="vector-caption"><span>Feature-space projection</span><span>{event ? new Date(event.ts_ms).toLocaleString() : "Awaiting Mirror observations"}</span></div>
  </div>;
}

export default function App() {
  const [tab, setTab] = useState<Tab>("archive");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [health, setHealth] = useState(false);
  const [cycle, setCycle] = useState<OperatingCycleSnapshot | null>(null);
  const [session, setSession] = useState<PaperSessionSnapshot | null>(null);
  const [history, setHistory] = useState<{ decisions: PaperMirrorEvent[]; trades: PaperTradeHistoryItem[] }>({ decisions: [], trades: [] });
  const [runs, setRuns] = useState<PaperSessionRun[]>([]);
  const [filter, setFilter] = useState("All");
  const [query, setQuery] = useState("");

  useEffect(() => {
    const refresh = () => {
      getQoinHealth().then((v) => setHealth(!!v)).catch(() => setHealth(false));
      getOperatingCycle().then(setCycle).catch(() => setCycle(null));
      getPaperSession().then(setSession).catch(() => setSession(null));
      getPaperSessionHistory(undefined, 100).then(setHistory).catch(() => setHistory({ decisions: [], trades: [] }));
      getPaperSessionRuns(20).then((v) => setRuns(v.runs)).catch(() => setRuns([]));
    };
    refresh();
    const timer = window.setInterval(refresh, 5000);
    return () => window.clearInterval(timer);
  }, []);

  const instruments = useMemo<Instrument[]>(() => {
    const byKey = new Map<string, Instrument>();
    for (const account of cycle?.config.paper_accounts ?? []) byKey.set(account.venue + ":" + account.symbol, {
      symbol: account.symbol, venue: account.venue, currency: account.quote_currency, initialCash: account.initial_cash, status: "Configured",
    });
    for (const account of session?.accounts ?? []) byKey.set(account.venue + ":" + account.symbol, {
      symbol: account.symbol, venue: account.venue, currency: account.quote_currency, initialCash: account.initial_cash,
      equity: account.equity, pnl: account.net_pnl, status: "Paper account",
    });
    for (const run of runs) for (const account of run.accounts) {
      const key = account.venue + ":" + account.symbol;
      const current = byKey.get(key);
      byKey.set(key, { ...current, symbol: account.symbol, venue: account.venue, currency: account.quote_currency,
        equity: current?.equity, pnl: current?.pnl ?? account.net_pnl, status: current?.status ?? "Archived" });
    }
    return [...byKey.values()].sort((a, b) => a.symbol.localeCompare(b.symbol));
  }, [cycle, session, runs]);

  const selectedEvents = selected ? history.decisions.filter((e) => e.symbol === selected.symbol && e.venue === selected.venue) : [];
  const latest = selectedEvents[0];
  const selectedTrades = selected ? history.trades.filter((t) => t.symbol === selected.symbol && t.venue === selected.venue) : [];
  const visibleDiscovery = discovery.filter((item) => (filter === "All" || item.category === filter) &&
    (item.symbol + item.name).toLowerCase().includes(query.toLowerCase()));

  function addToArchive(item: typeof discovery[number]) {
    setSelected({ symbol: item.symbol, venue: "Not configured", currency: item.symbol.endsWith("USDT") ? "USDT" : "USD", status: "Discovery only" });
  }

  return <main className="qoin-app">
    <header className="topbar">
      <a className="brand" href="/" aria-label="Qoin home"><span className="brand-mark">Q</span><span>QOIN</span></a>
      <div className="topbar-right"><span className="status"><i className={health ? "dot online" : "dot"} />{health ? "Engine connected" : "Research workspace"}</span><span className="avatar">Q</span></div>
    </header>

    {selected ? <section className="instrument-view">
      <button className="back-link" onClick={() => setSelected(null)}>← Back to {tab}</button>
      <div className="instrument-heading"><div><p className="eyebrow">INSTRUMENT SPACE / {selected.venue}</p><h1>{selected.symbol}</h1><p className="muted">{selected.currency} quote currency · {selected.status}</p></div><span className="instrument-mark">{selected.symbol.slice(0, 1)}</span></div>
      <div className="instrument-grid">
        <section className="surface vector-panel"><div className="section-heading"><div><p className="eyebrow">LIVE STATE REPRESENTATION</p><h2>Vector space</h2></div><span className="soft-pill">{latest?.action?.toUpperCase() ?? "NO SIGNAL"}</span></div>
          <VectorSpace event={latest} />
          <div className="vector-stats"><div><small>Magnitude</small><strong>{latest?.magnitude.toFixed(3) ?? "—"}</strong></div><div><small>Angle</small><strong>{latest?.angle_radians.toFixed(3) ?? "—"}</strong></div><div><small>Strategy</small><strong>{latest?.strategy_version ?? "—"}</strong></div></div>
          {!latest && <p className="empty-note">This instrument has no saved Mirror observations yet. Once its exact venue/symbol feed is configured, its feature state and decision history will appear here.</p>}
        </section>
        <aside className="surface side-panel"><p className="eyebrow">INSTRUMENT ACTIVITY</p><h2>Engine readout</h2>
          {latest ? <><div className="readout-row"><span>Decision</span><strong>{latest.action.toUpperCase()}</strong></div><div className="readout-row"><span>Execution</span><strong>{latest.execution}</strong></div><div className="readout-row"><span>Reason</span><strong>{latest.reason}</strong></div>{Object.entries(latest.features).map(([name, value]) => <div className="readout-row" key={name}><span>{name.replaceAll("_", " ")}</span><strong>{value.toFixed(3)}</strong></div>)}</> : <p className="muted">No decision trace recorded for this instrument yet.</p>}
        </aside>
      </div>
      <section className="surface activity-panel"><div className="section-heading"><div><p className="eyebrow">PERSISTENT RECORD</p><h2>Mirror history</h2></div><span className="muted">{selectedEvents.length} decisions · {selectedTrades.length} closed trades</span></div>
        {selectedEvents.slice(0, 12).map((event) => <article className="activity-row" key={event.event_id}><div><strong>{event.action.toUpperCase()}</strong><small>{new Date(event.ts_ms).toLocaleString()}</small></div><span>{event.reason}</span><small>Vector {event.decision_vector.map((v) => v.toFixed(2)).join(" · ")} · magnitude {event.magnitude.toFixed(3)}</small></article>)}
        {selectedTrades.slice(0, 8).map((trade, index) => <article className="activity-row" key={trade.entry_event_id + index}><div><strong>TRADE · {trade.net_pnl.toFixed(2)} {trade.quote_currency}</strong><small>{new Date(trade.exit_ts_ms).toLocaleString()}</small></div><span>Fees {trade.fees.toFixed(2)} · Return {trade.return_pct.toFixed(2)}%</span></article>)}
        {!selectedEvents.length && !selectedTrades.length && <p className="empty-note">No instrument activity to display yet.</p>}
      </section>
    </section> : <div className="workspace">
      <aside className="left-rail"><div className="rail-label">YOUR SPACE</div><button className={tab === "archive" ? "rail-link active" : "rail-link"} onClick={() => setTab("archive")}>◈ <span>Archive</span></button><button className={tab === "explore" ? "rail-link active" : "rail-link"} onClick={() => setTab("explore")}>⌕ <span>Explore</span></button>
        <div className="rail-summary"><small>YOUR INSTRUMENTS</small><strong>{instruments.length}</strong><span>Saved in Archive</span></div><div className="rail-safety"><i className="dot online" /> Paper research only<br /><small>Live order routing disabled</small></div>
      </aside>
      <section className="archive-column">
        <div className="page-heading"><div><p className="eyebrow">{tab === "archive" ? "YOUR MARKET COLLECTION" : "DISCOVER THE MARKETS"}</p><h1>{tab === "archive" ? "Archive" : "Explore"}</h1><p className="muted">{tab === "archive" ? "Your instruments, each with its own evolving vector space." : "Market stories and instruments worth investigating."}</p></div><span className="count-pill">{tab === "archive" ? instruments.length + " instruments" : "Discover"}</span></div>
        {tab === "archive" ? <>{instruments.length ? <div className="instrument-list">{instruments.map((item) => {
          const event = history.decisions.find((e) => e.symbol === item.symbol && e.venue === item.venue);
          return <button className="instrument-card" key={item.venue + ":" + item.symbol} onClick={() => setSelected(item)}>
            <span className="asset-avatar">{item.symbol.slice(0, 1)}</span><span className="asset-main"><strong>{item.symbol}</strong><small>{item.venue} · {item.currency}</small><span className="asset-state">{event ? "Vector updated " + new Date(event.ts_ms).toLocaleTimeString() : item.status}</span></span>
            <span className="asset-metrics"><strong>{item.pnl == null ? "—" : item.pnl.toFixed(2) + " " + item.currency}</strong><small>{event?.action?.toUpperCase() ?? "AWAITING DATA"}</small></span><span className="card-arrow">↗</span>
          </button>;
        })}</div> : <div className="empty-state"><div className="empty-orbit">◈</div><h2>Your Archive starts here</h2><p>Explore instruments and add the ones you want Qoin to observe. Each saved instrument opens its own vector workspace.</p><button className="blue-button" onClick={() => setTab("explore")}>Explore instruments</button></div>}</> :
          <div className="explore-feed">
            <div className="news-placeholder surface"><div className="news-art">Q</div><div><p className="eyebrow">MARKET BRIEFING</p><h2>News and market context</h2><p className="muted">The news provider is not connected yet. This space is reserved for sourced headlines, market-moving events and their instrument links.</p><span className="soft-pill">Awaiting news source</span></div></div>
            <div className="section-heading explore-title"><div><p className="eyebrow">INSTRUMENT DIRECTORY</p><h2>Find an instrument</h2></div></div>
            <div className="filter-row">{["All", "Forex", "Crypto", "Commodities", "Stocks"].map((name) => <button key={name} className={filter === name ? "filter-chip selected" : "filter-chip"} onClick={() => setFilter(name)}>{name}</button>)}</div>
            <input className="search-input" value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search instruments…" />
            <div className="discovery-list">{visibleDiscovery.map((item) => <article className="discovery-card" key={item.symbol}><span className="asset-avatar">{item.symbol.slice(0, 1)}</span><span className="asset-main"><strong>{item.symbol}</strong><small>{item.name} · {item.category}</small></span><button className="add-button" onClick={() => addToArchive(item)}>＋ Add</button></article>)}</div>
          </div>}
      </section>
      <aside className="explore-column"><div className="side-sticky"><p className="eyebrow">QOIN ENGINE</p><h2>{cycle?.phase?.replaceAll("_", " ") ?? "Research mode"}</h2><p className="muted">Your instruments are observed and evaluated through the operating cycle.</p><div className="cycle-steps"><div className={cycle?.phase === "observation" ? "step current" : "step"}><i /> <span><strong>Observation</strong><small>Market data collection</small></span></div><div className={cycle?.phase === "weight_evaluation" ? "step current" : "step"}><i /> <span><strong>Weight diagnostics</strong><small>Feature and vector review</small></span></div><div className={cycle?.phase === "paper_session" ? "step current" : "step"}><i /> <span><strong>Paper session</strong><small>Simulated execution only</small></span></div></div><div className="side-divider" /><p className="eyebrow">RECENT MIRROR</p>{history.decisions.slice(0, 4).map((event) => <div className="mini-event" key={event.event_id}><strong>{event.symbol} · {event.action.toUpperCase()}</strong><small>{new Date(event.ts_ms).toLocaleTimeString()} · {event.execution}</small></div>)}{!history.decisions.length && <p className="muted">Your decision trace will appear here as instruments produce observations.</p>}</div></aside>
    </div>}
    <nav className="floating-tabs" aria-label="Primary navigation"><button className={tab === "archive" ? "tab active" : "tab"} onClick={() => { setSelected(null); setTab("archive"); }}>◈ <span>Archive</span></button><button className={tab === "explore" ? "tab active" : "tab"} onClick={() => { setSelected(null); setTab("explore"); }}>⌕ <span>Explore</span></button></nav>
    <footer className="app-footer"><span>QOIN / MARKET INTELLIGENCE</span><span>PAPER RESEARCH · NO LIVE EXECUTION</span></footer>
  </main>;
}
