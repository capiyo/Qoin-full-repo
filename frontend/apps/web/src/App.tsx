import { Fragment, useEffect, useMemo, useState } from "react";
import { getInstrumentVector, getInstrumentVectorHistory, getQoinEvents, getQoinCalendar, getQoinHealth, getQoinInstruments, getQoinNews, getQoinStrategy, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinHealth, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent, type QoinStrategy } from "@qoin/core";

type Instrument = {
  symbol: string;
  name: string;
  market: QoinInstrument["asset_class"];
  engineSymbol: string;
  description: string;
  venue: string;
  provider: string;
  providerStatus?: string;
  archived: boolean;
  color: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", engineSymbol: "BTCUSDT", name: "Bitcoin", market: "Crypto", description: "Bitcoin perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, color: "#f59e0b" },
  { symbol: "ETH/USD", engineSymbol: "ETHUSDT", name: "Ethereum", market: "Crypto", description: "Ether perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, color: "#6477df" },
  { symbol: "SOL/USD", engineSymbol: "SOLUSDT", name: "Solana", market: "Crypto", description: "Solana perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, color: "#8b72e8" },
  { symbol: "NVDA", engineSymbol: "NVDA", name: "NVIDIA", market: "Equities", description: "Semiconductors · NASDAQ", venue: "NASDAQ", provider: "Finnhub", archived: false, color: "#76a85a" },
  { symbol: "EUR/USD", engineSymbol: "OANDA:EUR_USD", name: "Euro / US Dollar", market: "FX", description: "Major FX pair", venue: "OANDA", provider: "Finnhub", archived: false, color: "#4d91d8" },
  { symbol: "AAPL", engineSymbol: "AAPL", name: "Apple", market: "Equities", description: "Consumer technology · NASDAQ", venue: "NASDAQ", provider: "Finnhub", archived: false, color: "#8895a7" },
];

type Tab = "Archive" | "Explore";

const QOIN_FEATURE_SHORT_NAMES = ["OFI", "FUND", "DEPTH", "VOL", "EVENT", "LIQ"] as const;

function VectorMap({ decision, history }: { decision: QoinVectorDecision | null; history: QoinVectorHistory[] }) {
  const bullish = decision?.decision_vector?.[0] ?? 0;
  const bearish = decision?.decision_vector?.[1] ?? 0;
  const points = [...history, ...(decision?.decision_vector ? [{ decision_vector: decision.decision_vector } as QoinVectorHistory] : [])];
  const max = Math.max(0.35, ...points.flatMap((item) => item.decision_vector || [0, 0]));
  const project = (item: QoinVectorHistory) => [10 + (item.decision_vector[0] / max) * 80, 90 - (item.decision_vector[1] / max) * 80] as const;
  const x = 10 + (bullish / max) * 80;
  const y = 90 - (bearish / max) * 80;
  const trajectory = points.map((item) => project(item).join(",")).join(" ");
  return (
    <div className="vector-map">
      <svg viewBox="0 0 100 100" role="img" aria-label="Qoin engine bullish and bearish decision vector">
        <defs><pattern id="grid" width="10" height="10" patternUnits="userSpaceOnUse"><path d="M 10 0 L 0 0 0 10" fill="none" stroke="#e9eef6" strokeWidth=".45" /></pattern></defs>
        <rect width="100" height="100" fill="url(#grid)" />
        <path d="M10 8V90H94" fill="none" stroke="#cbd5e1" strokeWidth=".8" />
        <path d="M10 90L94 90 M10 90L10 8" stroke="#cbd5e1" strokeWidth=".5" strokeDasharray="2 2" />
        {points.length > 1 && <polyline points={trajectory} fill="none" stroke="#9bb9f3" strokeWidth="1.1" strokeLinejoin="round" />}
        {history.map((item, index) => { const [px, py] = project(item); return <circle key={item.stored_at || index} cx={px} cy={py} r="1.5" fill="#9bb9f3" />; })}
        {decision?.decision_vector ? <><line x1="10" y1="90" x2={x} y2={y} stroke="#2867dc" strokeWidth="1.8" /><circle cx={x} cy={y} r="3.8" fill="#2867dc" stroke="white" strokeWidth="1.2" /><circle cx="10" cy="90" r="2" fill="#94a3b8" /></> : <text x="50" y="48" textAnchor="middle" fill="#94a3b8" fontSize="5">NO VECTOR YET</text>}
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
      {decision && <><polygon points={polygon} fill="#3475e8" fillOpacity=".16" stroke="#2867dc" strokeWidth="1.2" />{coordinates.map(([x,y],i) => <circle key={labels[i]} cx={x} cy={y} r="1.8" fill="#2867dc" />)}</>}
      {!decision && <text x="50" y="52" textAnchor="middle" fill="#94a3b8" fontSize="5">NO FEATURE VECTOR</text>}
    </svg>
    <div className="feature-space-values">{labels.map((label,index) => <span key={label}>{label} <strong>{values[index].toFixed(3)}</strong></span>)}</div>
  </section>;
}

function InstrumentCard({ item, onOpen, action, live }: { item: Instrument; onOpen: () => void; action?: { label: string; onClick: () => void }; live?: { state?: QoinMarketState; decision?: QoinVectorDecision } }) {
  return (
    <article className="instrument-card">
      <button className="card-main" onClick={onOpen} aria-label={`Open ${item.name} vector space`}>
        <span className="asset-mark" style={{ background: item.color }}>{item.symbol.slice(0, 1)}</span>
        <span className="asset-copy"><strong>{item.name}</strong><small>{item.symbol} <i>·</i> {item.market}</small></span>
        <span className="open-arrow" aria-hidden="true">↗</span>
      </button>
      <p className="asset-description">{item.description}</p><p className="asset-provider">{item.venue} · {item.provider} · {item.providerStatus || "provider status unknown"}</p><div className="asset-live-row"><strong>{live?.state?.last_price != null ? live.state.last_price.toLocaleString(undefined,{maximumFractionDigits:8}) : "—"}</strong><span>{live?.decision?.action?.toUpperCase() || "NO LIVE DATA"}</span></div>
      {action && <button className="text-action" onClick={action.onClick}>{action.label}</button>}
    </article>
  );
}

export default function App() {
  const [tab, setTab] = useState<Tab>("Archive");
  const [instruments, setInstruments] = useState(seed);
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [health, setHealth] = useState<QoinHealth | null>(null);
  const [strategy, setStrategy] = useState<QoinStrategy | null>(null);
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [vectorLoading, setVectorLoading] = useState(false);
  const [vectorError, setVectorError] = useState("");
  const [vectorHistory, setVectorHistory] = useState<QoinVectorHistory[]>([]);
  const [rawEvents, setRawEvents] = useState<QoinMarketEvent[]>([]);
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

  useEffect(() => { getQoinHealth().then(setHealth).catch(() => setHealth(null)); getQoinStrategy().then(setStrategy).catch(() => setStrategy(null)); }, []);
  useEffect(() => {
    let active = true;
    getQoinInstruments().then(({ items }) => {
      if (!active) return;
      const colors = ["#e6a23c", "#6477df", "#8b72e8", "#4d91d8", "#76a85a", "#8895a7"];
      setInstruments(items.map((item, index) => ({
        symbol: item.symbol, engineSymbol: item.engine_symbol, name: item.name,
        market: item.asset_class, description: item.description, venue: item.venue,
        provider: item.provider, providerStatus: item.provider_status,
        archived: ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol), color: colors[index % colors.length],
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
    if (!selected) { setVectorHistory([]); setRawEvents([]); setMarketState(null); setLastEvent(null); return; }
    let active = true;
    setVectorHistory([]);
    setRawEvents([]);
    getInstrumentVectorHistory(selected.engineSymbol, 500).then((result) => { if (active) setVectorHistory(result.items); }).catch(() => undefined);
    getQoinEvents(selected.engineSymbol, 1000).then((result) => { if (active) setRawEvents(result.items.slice().reverse()); }).catch(() => undefined);
    return () => { active = false; };
  }, [selected?.engineSymbol]);

  useEffect(() => {
    const symbols = instruments.map((item) => item.engineSymbol);
    if (!symbols.length) return;
    let active = true;
    const unsubscribe = subscribeQoinStream(symbols, (update) => {
      if (!active) return;
      if (update.type === "market.update" && update.symbol) {
        const symbol = update.symbol.toUpperCase();
        setLiveBySymbol((current) => ({ ...current, [symbol]: { state: update.state, decision: update.decision, event: update.event } }));
        if (selected?.engineSymbol.toUpperCase() === symbol) {
          if (update.state) setMarketState(update.state);
          if (update.event) {
            setLastEvent(update.event);
            setRawEvents((current) => [{ ...update.event, event_id: update.event?.event_id || String(update.server_ts_ms || Date.now()), venue: update.event?.venue || "stream", symbol, event_type: update.event?.event_type || "unknown", ts_ms: update.event?.ts_ms || update.server_ts_ms || Date.now() } as QoinMarketEvent, ...current].slice(0, 200));
          }
          if (update.decision) {
            setVector(update.decision);
            setVectorHistory((current) => [...current.slice(-119), {
              symbol: update.decision!.symbol, decision_vector: update.decision!.decision_vector || [0, 0],
              magnitude: update.decision!.magnitude || 0, angle_radians: update.decision!.angle_radians || 0,
              action: update.decision!.action || "wait", strategy_version: update.decision!.strategy_version,
              stored_at: new Date(update.server_ts_ms || Date.now()).toISOString(),
            }]);
          }
        }
      }
      if (update.type === "stream.gap") setStreamStatus("gap — refresh required");
    }, setStreamStatus);
    return () => { active = false; unsubscribe(); };
  }, [instruments, selected?.engineSymbol]);

  useEffect(() => {
    const current = selected ? liveBySymbol[selected.engineSymbol.toUpperCase()] : undefined;
    setMarketState(current?.state || null);
    setLastEvent(current?.event || null);
  }, [selected?.engineSymbol, liveBySymbol]);

  const archived = useMemo(() => instruments.filter((item) => item.archived), [instruments]);
  const discovered = useMemo(() => instruments.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [instruments, market, query]);

  function toggleArchive(symbol: string) {
    setInstruments((items) => items.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
  }

  return (
    <main className="qoin-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Qoin home"><span className="brand-mark">q</span><span>qoin</span></a>
        <div className="topbar-right"><span className={`connection-dot ${health ? "connected" : ""}`} /><span>{health ? "Research engine connected" : "Research workspace"}</span><button className="avatar">Q</button></div>
      </header>

      <section className="welcome">
        <div><p className="eyebrow">YOUR MARKET WORKSPACE</p><h1>See the market<br /><span>as a system.</span></h1><p className="welcome-copy">Keep the instruments that matter close. Explore the forces shaping each one.</p></div>
        <div className="welcome-orbit" aria-hidden="true"><span /><span /><span /><b>q</b></div>
      </section>

      <nav className="main-tabs" aria-label="Main navigation">
        {(["Archive", "Explore"] as Tab[]).map((name) => <button key={name} className={tab === name ? "tab active" : "tab"} onClick={() => { setTab(name); setSelected(null); }}>{name}<span>{name === "Archive" ? archived.length : instruments.length - archived.length}</span></button>)}
      </nav>

      {tab === "Archive" ? (
        <section className="content-section">
          <div className="section-heading"><div><p className="eyebrow">YOUR COLLECTION</p><h2>Archive</h2><p>Every instrument you follow, in one place.</p></div><span className="count-label">{archived.length} INSTRUMENTS</span></div>
          {archived.length ? <div className="instrument-grid">{archived.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => setSelected(item)} live={liveBySymbol[item.engineSymbol.toUpperCase()]} />)}</div> : <div className="empty-state"><strong>Your archive is clear.</strong><p>Explore instruments and add the ones you want to follow.</p><button className="primary-button" onClick={() => setTab("Explore")}>Explore instruments</button></div>}
        </section>
      ) : (
        <section className="content-section">
          <div className="section-heading"><div><p className="eyebrow">DISCOVERY</p><h2>Explore</h2><p>Find instruments to bring into your archive.</p></div><span className="count-label">CRYPTO · EQUITIES · FX · INDICES · COMMODITIES</span></div>
          <label className="search-box"><span>⌕</span><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search instruments or symbols" /></label>
          <div className="filter-row">{["All", "Crypto", "Equities", "FX", "Indices", "Commodities"].map((name) => <button key={name} className={market === name ? "filter active" : "filter"} onClick={() => setMarket(name)}>{name}</button>)}</div>
          {discovered.length ? <div className="instrument-grid">{discovered.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => setSelected(item)} live={liveBySymbol[item.engineSymbol.toUpperCase()]} action={{ label: "＋ Add to archive", onClick: () => toggleArchive(item.symbol) }} />)}</div> : <div className="empty-state"><strong>No instruments found.</strong><p>Try another name or market.</p></div>}
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
        </section>
      )}

      <footer className="footer"><span>QOIN · MARKET INTELLIGENCE</span><span>RESEARCH MODE · NO LIVE EXECUTION</span></footer>

      {selected && <div className="modal-backdrop" role="presentation" onMouseDown={(e) => { if (e.target === e.currentTarget) setSelected(null); }}><section className="vector-modal" role="dialog" aria-modal="true" aria-labelledby="vector-title">
        <header className="modal-header"><div><p className="eyebrow">INSTRUMENT SPACE</p><h2 id="vector-title">{selected.name}</h2><p>{selected.symbol} <i>·</i> {selected.market}</p></div><button className="close-button" onClick={() => setSelected(null)} aria-label="Close vector space">×</button></header>
        <div className="modal-body">
          <div className="vector-intro"><div><strong>Force map</strong><p>Latest persisted Axum decision vector on bullish and bearish axes.</p></div><span className="preview-tag">{streamStatus.toUpperCase()}</span></div>
          <VectorMap decision={vector} history={vectorHistory} />
          <FeatureSpace decision={vector} />
          <section className="live-state-panel">
            <div className="live-state-heading"><strong>Live market state</strong><span>{lastEvent?.event_type || "awaiting event"}</span></div>
            <div className="live-state-grid">
              <div><span>Last price</span><strong>{marketState?.last_price?.toLocaleString(undefined,{maximumFractionDigits:8}) ?? "—"}</strong></div>
              <div><span>Spread (bps)</span><strong>{marketState?.spread_bps?.toFixed(3) ?? "—"}</strong></div>
              <div><span>Order flow imbalance</span><strong>{marketState?.order_flow_imbalance?.toFixed(4) ?? "—"}</strong></div>
              <div><span>Funding rate</span><strong>{marketState?.funding_rate?.toFixed(6) ?? "—"}</strong></div>
              <div><span>Open interest</span><strong>{marketState?.open_interest?.toLocaleString() ?? "—"}</strong></div>
              <div><span>Liquidations</span><strong>{marketState?.liquidation_events ?? "—"}</strong></div>
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
            <div className="attention-heading"><strong>Active strategy map</strong><span>{strategy?.version || "Strategy unavailable"} · {strategy?.status || "unknown status"}</span></div>
            {strategy ? <>
              <div className="strategy-gates">
                <div><span>Angular threshold</span><strong>{strategy.angular_threshold_degrees.toFixed(2)}°</strong></div>
                <div><span>Magnitude threshold</span><strong>{strategy.magnitude_threshold.toFixed(3)}</strong></div>
                <div><span>Maximum spread</span><strong>{strategy.max_spread_bps.toFixed(2)} bps</strong></div>
                <div><span>Attention scale</span><strong>{strategy.attention_scale.toFixed(3)}</strong></div>
              </div>
              <div className="strategy-weights">{QOIN_FEATURE_NAMES.map((name) => <div className="strategy-weight-row" key={name}>
                <strong>{name.replaceAll("_", " ")}</strong>
                <span>Bullish <b>{strategy.bullish_weights[name]?.toFixed(3) ?? "—"}</b></span>
                <span>Bearish <b>{strategy.bearish_weights[name]?.toFixed(3) ?? "—"}</b></span>
                <span>Scale <b>{strategy.feature_scales[name]?.toFixed(3) ?? "—"}</b></span>
              </div>)}</div>
              <p className="disclaimer">MongoDB-backed active strategy parameters. Last update: {new Date(strategy.updated_at_ms).toLocaleString()}.</p>
            </> : <p className="provider-empty">The active strategy map could not be loaded.</p>}
          </section>
          <section className="raw-tape">
            <button className="raw-tape-toggle" onClick={() => setShowRawEvents((value) => !value)}>Raw event tape · {rawEvents.length} observations <span>{showRawEvents ? "Hide" : "Show"}</span></button>
            {showRawEvents && <div className="raw-event-list">{rawEvents.slice(0,200).map((event) => <details key={event.event_id}><summary><span>{event.event_type.toUpperCase()}</span><b>{event.price ?? event.funding_rate ?? "—"}</b><small>{new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</small></summary><pre>{JSON.stringify(event,null,2)}</pre></details>)}</div>}
          </section>
          <p className="disclaimer">{vector.reason || "Engine research output. It is not an order or a profitability claim."}</p>
            </>
          ) : <p className="disclaimer">{vectorError || "No persisted engine vector is available for this symbol yet."}</p>}
          <p className="disclaimer">The plotted arrow is the persisted research decision vector. Feature inputs and attention links are shown above. This is not an order or a profitability claim.</p>
        </div>
        <footer className="modal-footer"><button className="secondary-button" onClick={() => setSelected(null)}>Close</button><button className="primary-button" onClick={() => toggleArchive(selected.symbol)}>{selected.archived ? "Remove from archive" : "Add to archive"}</button></footer>
      </section></div>}
    </main>
  );
}
