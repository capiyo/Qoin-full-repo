import { useEffect, useMemo, useState } from "react";
import { getQoinHealth, type QoinHealth } from "@qoin/core";

type Instrument = {
  symbol: string;
  name: string;
  market: "Crypto" | "Equities" | "FX";
  description: string;
  archived: boolean;
  color: string;
  vector: { flow: number; liquidity: number; volatility: number; positioning: number };
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", name: "Bitcoin", market: "Crypto", description: "Digital asset · USD", archived: true, color: "#f59e0b", vector: { flow: 0.72, liquidity: 0.48, volatility: 0.64, positioning: -0.22 } },
  { symbol: "ETH/USD", name: "Ethereum", market: "Crypto", description: "Smart-contract network · USD", archived: true, color: "#6477df", vector: { flow: 0.36, liquidity: 0.62, volatility: 0.42, positioning: 0.18 } },
  { symbol: "SOL/USD", name: "Solana", market: "Crypto", description: "Layer-one network · USD", archived: true, color: "#8b72e8", vector: { flow: -0.18, liquidity: 0.24, volatility: 0.78, positioning: 0.52 } },
  { symbol: "NVDA", name: "NVIDIA", market: "Equities", description: "Semiconductors · NASDAQ", archived: false, color: "#76a85a", vector: { flow: 0.58, liquidity: 0.82, volatility: 0.46, positioning: 0.31 } },
  { symbol: "EUR/USD", name: "Euro / US Dollar", market: "FX", description: "Major currency pair", archived: false, color: "#4d91d8", vector: { flow: 0.12, liquidity: 0.91, volatility: 0.19, positioning: -0.08 } },
  { symbol: "AAPL", name: "Apple", market: "Equities", description: "Consumer technology · NASDAQ", archived: false, color: "#8895a7", vector: { flow: 0.28, liquidity: 0.86, volatility: 0.27, positioning: 0.14 } },
];

type Tab = "Archive" | "Explore";

function VectorMap({ instrument }: { instrument: Instrument }) {
  const v = instrument.vector;
  const points = [
    { label: "Order flow", x: 50 + v.flow * 34, y: 50 - v.flow * 22, color: "#2867dc" },
    { label: "Liquidity", x: 50 + v.liquidity * 28, y: 50 + v.liquidity * 27, color: "#60a5fa" },
    { label: "Volatility", x: 50 - v.volatility * 32, y: 50 - v.volatility * 26, color: "#8b5cf6" },
    { label: "Positioning", x: 50 + v.positioning * 35, y: 50 - v.positioning * 30, color: "#0f9f91" },
  ];
  return (
    <div className="vector-map">
      <svg viewBox="0 0 100 100" role="img" aria-label={`Illustrative vector-space map for ${instrument.symbol}`}>
        <defs><pattern id="grid" width="10" height="10" patternUnits="userSpaceOnUse"><path d="M 10 0 L 0 0 0 10" fill="none" stroke="#e9eef6" strokeWidth=".45" /></pattern></defs>
        <rect width="100" height="100" fill="url(#grid)" />
        <path d="M50 4V96 M4 50H96" stroke="#cbd5e1" strokeWidth=".7" strokeDasharray="2 2" />
        <circle cx="50" cy="50" r="2.2" fill="#94a3b8" />
        {points.map((p) => <g key={p.label}><line x1="50" y1="50" x2={p.x} y2={p.y} stroke={p.color} strokeWidth="1.1" strokeDasharray="2 1.5" opacity=".8" /><circle cx={p.x} cy={p.y} r="2.7" fill={p.color} stroke="white" strokeWidth="1" /></g>)}
        <circle cx="50" cy="50" r="5.5" fill={instrument.color} stroke="white" strokeWidth="1.5" />
      </svg>
      <div className="axis-label axis-top">FLOW / MOMENTUM</div>
      <div className="axis-label axis-bottom">LIQUIDITY / DEPTH</div>
      <div className="axis-label axis-left">NEGATIVE</div>
      <div className="axis-label axis-right">POSITIVE</div>
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
          <div className="vector-intro"><div><strong>Force map</strong><p>How this instrument can be represented across interacting market dimensions.</p></div><span className="preview-tag">VECTOR PREVIEW</span></div>
          <VectorMap instrument={selected} />
          <div className="vector-legend">{[["#2867dc","Order flow"],["#60a5fa","Liquidity"],["#8b5cf6","Volatility"],["#0f9f91","Positioning"]].map(([color,label]) => <span key={label}><i style={{ background: color }} />{label}</span>)}</div>
          <div className="dimension-list">{Object.entries(selected.vector).map(([key,value]) => <div key={key}><span>{key.replace("_", " ")}</span><strong>{value > 0 ? "+" : ""}{value.toFixed(2)}</strong><div className="dimension-track"><i style={{ width: `${Math.abs(value) * 100}%`, marginLeft: value < 0 ? "auto" : 0 }} /></div></div>)}</div>
          <p className="disclaimer">Illustrative coordinates only. Live, timestamped observations and engine-produced embeddings are not connected to this preview. This is not a trading recommendation.</p>
        </div>
        <footer className="modal-footer"><button className="secondary-button" onClick={() => setSelected(null)}>Close</button><button className="primary-button" onClick={() => toggleArchive(selected.symbol)}>{selected.archived ? "Remove from archive" : "Add to archive"}</button></footer>
      </section></div>}
    </main>
  );
}
