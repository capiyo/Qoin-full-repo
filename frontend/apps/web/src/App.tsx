import { useEffect, useMemo, useState } from "react";
import { getInstrumentVector, getQoinHealth, QOIN_FEATURE_NAMES, type QoinHealth, type QoinVectorDecision } from "@qoin/core";

type Instrument = {
  symbol: string;
  name: string;
  market: "Crypto" | "Equities" | "FX";
  description: string;
  archived: boolean;
  color: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", name: "Bitcoin", market: "Crypto", description: "Digital asset · USD", archived: true, color: "#f59e0b" },
  { symbol: "ETH/USD", name: "Ethereum", market: "Crypto", description: "Smart-contract network · USD", archived: true, color: "#6477df" },
  { symbol: "SOL/USD", name: "Solana", market: "Crypto", description: "Layer-one network · USD", archived: true, color: "#8b72e8" },
  { symbol: "NVDA", name: "NVIDIA", market: "Equities", description: "Semiconductors · NASDAQ", archived: false, color: "#76a85a" },
  { symbol: "EUR/USD", name: "Euro / US Dollar", market: "FX", description: "Major currency pair", archived: false, color: "#4d91d8" },
  { symbol: "AAPL", name: "Apple", market: "Equities", description: "Consumer technology · NASDAQ", archived: false, color: "#8895a7" },
];

type Tab = "Archive" | "Explore";

function VectorMap({ decision }: { decision: QoinVectorDecision | null }) {
  const bullish = decision?.decision_vector?.[0] ?? 0;
  const bearish = decision?.decision_vector?.[1] ?? 0;
  const max = Math.max(0.35, bullish, bearish);
  const x = 10 + (bullish / max) * 80;
  const y = 90 - (bearish / max) * 80;
  return (
    <div className="vector-map">
      <svg viewBox="0 0 100 100" role="img" aria-label="Qoin engine bullish and bearish decision vector">
        <defs><pattern id="grid" width="10" height="10" patternUnits="userSpaceOnUse"><path d="M 10 0 L 0 0 0 10" fill="none" stroke="#e9eef6" strokeWidth=".45" /></pattern></defs>
        <rect width="100" height="100" fill="url(#grid)" />
        <path d="M10 8V90H94" fill="none" stroke="#cbd5e1" strokeWidth=".8" />
        <path d="M10 90L94 90 M10 90L10 8" stroke="#cbd5e1" strokeWidth=".5" strokeDasharray="2 2" />
        {decision?.decision_vector ? <><line x1="10" y1="90" x2={x} y2={y} stroke="#2867dc" strokeWidth="1.8" /><circle cx={x} cy={y} r="3.8" fill="#2867dc" stroke="white" strokeWidth="1.2" /><circle cx="10" cy="90" r="2" fill="#94a3b8" /></> : <text x="50" y="48" textAnchor="middle" fill="#94a3b8" fontSize="5">NO VECTOR YET</text>}
      </svg>
      <div className="axis-label axis-top">BEARISH COMPONENT ↑</div>
      <div className="axis-label axis-bottom">BULLISH COMPONENT →</div>
    </div>
  );
}

function InstrumentCard({ item, onOpen, action }: { item: Instrument; onOpen: () => void; action?: { label: string; onClick: () => void } }) {
  return (
    <article className="instrument-card">
      <button className="card-main" onClick={onOpen} aria-label={`Open ${item.name} vector space`}>
        <span className="asset-mark" style={{ background: item.color }}>{item.symbol.slice(0, 1)}</span>
        <span className="asset-copy"><strong>{item.name}</strong><small>{item.symbol} <i>·</i> {item.market}</small></span>
        <span className="open-arrow" aria-hidden="true">↗</span>
      </button>
      <p className="asset-description">{item.description}</p>
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
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [vectorLoading, setVectorLoading] = useState(false);
  const [vectorError, setVectorError] = useState("");

  useEffect(() => { getQoinHealth().then(setHealth).catch(() => setHealth(null)); }, []);

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
          {archived.length ? <div className="instrument-grid">{archived.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => setSelected(item)} />)}</div> : <div className="empty-state"><strong>Your archive is clear.</strong><p>Explore instruments and add the ones you want to follow.</p><button className="primary-button" onClick={() => setTab("Explore")}>Explore instruments</button></div>}
        </section>
      ) : (
        <section className="content-section">
          <div className="section-heading"><div><p className="eyebrow">DISCOVERY</p><h2>Explore</h2><p>Find instruments to bring into your archive.</p></div><span className="count-label">MARKET UNIVERSE</span></div>
          <label className="search-box"><span>⌕</span><input value={query} onChange={(e) => setQuery(e.target.value)} placeholder="Search instruments or symbols" /></label>
          <div className="filter-row">{["All", "Crypto", "Equities", "FX"].map((name) => <button key={name} className={market === name ? "filter active" : "filter"} onClick={() => setMarket(name)}>{name}</button>)}</div>
          {discovered.length ? <div className="instrument-grid">{discovered.map((item) => <InstrumentCard key={item.symbol} item={item} onOpen={() => setSelected(item)} action={{ label: "＋ Add to archive", onClick: () => toggleArchive(item.symbol) }} />)}</div> : <div className="empty-state"><strong>No instruments found.</strong><p>Try another name or market.</p></div>}
          <div className="explore-note"><strong>Discovery feed</strong><p>News, macro events and instrument universes will appear here as their data sources are connected. No simulated headlines are shown.</p></div>
        </section>
      )}

      <footer className="footer"><span>QOIN · MARKET INTELLIGENCE</span><span>RESEARCH MODE · NO LIVE EXECUTION</span></footer>

      {selected && <div className="modal-backdrop" role="presentation" onMouseDown={(e) => { if (e.target === e.currentTarget) setSelected(null); }}><section className="vector-modal" role="dialog" aria-modal="true" aria-labelledby="vector-title">
        <header className="modal-header"><div><p className="eyebrow">INSTRUMENT SPACE</p><h2 id="vector-title">{selected.name}</h2><p>{selected.symbol} <i>·</i> {selected.market}</p></div><button className="close-button" onClick={() => setSelected(null)} aria-label="Close vector space">×</button></header>
        <div className="modal-body">
          <div className="vector-intro"><div><strong>Force map</strong><p>How this instrument can be represented across interacting market dimensions.</p></div><span className="preview-tag">{vector ? "LIVE ENGINE VECTOR" : "ENGINE VECTOR"}</span></div>
          <VectorMap decision={vector} />
          {vectorLoading ? <p className="disclaimer">Loading the latest persisted engine vector…</p> : vector ? (
            <>
              <div className="engine-metrics">
                <div><span>Decision</span><strong>{vector.action?.toUpperCase() || "WAIT"}</strong></div>
                <div><span>Magnitude</span><strong>{vector.magnitude?.toFixed(3) ?? "—"}</strong></div>
                <div><span>Angle</span><strong>{vector.angle_radians?.toFixed(3) ?? "—"} rad</strong></div>
                <div><span>Strategy</span><strong>{vector.strategy_version || "—"}</strong></div>
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
              <p className="disclaimer">{vector.reason || "Engine research output. It is not an order or a profitability claim."}</p>
            </>
          ) : <p className="disclaimer">{vectorError || "No persisted engine vector is available for this symbol yet."}</p>}
          <p className="disclaimer">Illustrative coordinates only. Live, timestamped observations and engine-produced embeddings are not connected to this preview. This is not a trading recommendation.</p>
        </div>
        <footer className="modal-footer"><button className="secondary-button" onClick={() => setSelected(null)}>Close</button><button className="primary-button" onClick={() => toggleArchive(selected.symbol)}>{selected.archived ? "Remove from archive" : "Add to archive"}</button></footer>
      </section></div>}
    </main>
  );
}
