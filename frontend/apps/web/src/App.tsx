import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import type { User } from "firebase/auth";
import type { FormEvent, ChangeEvent } from "react";
import { signInQoinUser, registerQoinUser, signOutQoinUser, subscribeQoinAuth, uploadQoinAvatar, getQoinArchivedSymbols, saveQoinArchivedSymbols } from "@qoin/core";
import { getInstrumentVector, getInstrumentVectorHistory, getQoinEvents, getQoinCalendar, getQoinHealth, getQoinInstruments, getQoinNews, getQoinStrategy, getQoinAlgorithmCatalogue, runQoinBacktest, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinAlgorithmCatalogue, type QoinHealth, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent, type QoinStrategy, type QoinBacktestResult } from "@qoin/core";

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
type QoinNotification = { id: string; symbol: string; title: string; body: string; kind: "signal" | "liquidation"; createdAt: number; read: boolean; details: string };

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

function BacktestPanel({ symbol, venue, authenticated, onRequireAuth }: { symbol: string; venue: string; authenticated: boolean; onRequireAuth: () => void }) {
  const [capital,setCapital]=useState("10000");
  const [fee,setFee]=useState("10");
  const [slippage,setSlippage]=useState("5");
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
      if(from && !Number.isFinite(fromMs)) throw new Error("Invalid start date");
      if(to && !Number.isFinite(toMs)) throw new Error("Invalid end date");
      const output=await runQoinBacktest({symbol,venue,from_ts_ms:fromMs,to_ts_ms:toMs,
        initial_capital:Number(capital),fee_bps:Number(fee),slippage_bps:Number(slippage),limit:10000});
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
        <div><span>Net funding (received +)</span><strong>{result.net_funding_cashflow.toFixed(4)}</strong></div>
        <div><span>Funding settlements</span><strong>{result.funding_settlements}</strong></div>
      </div>
      <p className="disclaimer">Strategy {result.strategy_key}@{result.strategy_version} · {result.venue} · {new Date(result.from_ts_ms).toLocaleString()} → {new Date(result.to_ts_ms).toLocaleString()}</p>
      <div className="trace-table-wrap"><table className="trace-table"><thead><tr><th>Time</th><th>Fill</th><th>Price</th><th>Quantity</th><th>Fee</th><th>Slippage</th><th>Realized P/L</th><th>Equity</th><th>Reason</th></tr></thead>
      <tbody>{result.fills.map((fill,index)=><tr key={fill.event_id+"-"+index}><td>{new Date(fill.ts_ms).toLocaleString()}</td><td>{fill.action}</td><td>{fill.price.toPrecision(8)}</td><td>{fill.quantity.toPrecision(6)}</td><td>{fill.fee.toFixed(4)}</td><td>{fill.slippage_cost.toFixed(4)}</td><td>{fill.realized_pnl.toFixed(4)}</td><td>{fill.equity_after.toFixed(4)}</td><td>{fill.reason}</td></tr>)}</tbody></table></div>
      <details className="trace-details"><summary>Replay assumptions and full result</summary><ul>{result.assumptions.map((item,index)=><li key={index}>{item}</li>)}</ul><pre>{JSON.stringify(result,null,2)}</pre></details>
    </>}
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
  const [firebaseUser, setFirebaseUser] = useState<User | null>(null);
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
  const pendingArchiveSymbolRef = useRef<string | null>(null);
  const [instruments, setInstruments] = useState(seed);
  const [selected, setSelected] = useState<Instrument | null>(null);
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
        setAvatarUrl(null);
        archivedSymbolsRef.current = null;
        if (!user) {
          setInstruments((items) => items.map((item) => ({ ...item, archived: ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol) })));
          return;
        }
        void getQoinArchivedSymbols().then((symbols) => {
          if (requestGeneration !== generation) return;
          const pendingSymbol = pendingArchiveSymbolRef.current;
          const nextSymbols = pendingSymbol && !symbols.includes(pendingSymbol) ? [...symbols, pendingSymbol] : symbols;
          pendingArchiveSymbolRef.current = null;
          archivedSymbolsRef.current = nextSymbols;
          setInstruments((items) => items.map((item) => ({ ...item, archived: nextSymbols.includes(item.symbol) })));
          if (pendingSymbol && nextSymbols !== symbols) {
            void saveQoinArchivedSymbols(nextSymbols).catch((error) => setAuthError(error instanceof Error ? error.message : "Could not save your archive."));
          }
        }).catch((error) => {
          if (requestGeneration === generation) setAuthError(error instanceof Error ? error.message : "Could not load your archive.");
        });
      });
    } catch {
      setFirebaseUser(null);
      setAuthError("Firebase is not configured. Add the VITE_FIREBASE_* values before signing in.");
      return undefined;
    }
  }, []);
  function requireAuthentication() { setAccountOpen(true); setAuthPanelOpen(true); setAuthError(firebaseUser ? "" : "Sign in to use this action."); }
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
  const decisionStrategy = vector?.strategy_parameters || strategy;
  const discovered = useMemo(() => instruments.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [instruments, market, query]);

  function toggleArchive(symbol: string) {
    if (!firebaseUser || archivedSymbolsRef.current === null) {
      pendingArchiveSymbolRef.current = symbol;
      if (!firebaseUser) requireAuthentication();
      else setAuthError("Your archive is still syncing. This change will be applied when it finishes loading.");
      return;
    }
    const target = instruments.find((item) => item.symbol === symbol);
    if (!target) return;
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

  return (
    <main className="qoin-shell">
      <header className="topbar">
        <a className="brand" href="/" aria-label="Qoin home"><span className="brand-mark">q</span><span>qoin</span></a>
        <div className="topbar-right"><span className={"connection-dot " + (health ? "connected" : "")} /><span>{health ? "Research engine connected" : "Research workspace"}</span><button className="avatar" aria-label="Account menu" aria-expanded={accountOpen} onClick={() => setAccountOpen((value) => !value)}>{(avatarUrl || firebaseUser?.photoURL) ? <img src={avatarUrl || firebaseUser?.photoURL || ""} alt="" /> : (firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</button></div>
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
              <BacktestPanel symbol={selected.engineSymbol} venue={selected.engineVenue} authenticated={Boolean(firebaseUser)} onRequireAuth={requireAuthentication} />
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
        </div>
        <footer className="modal-footer"><button className="secondary-button" onClick={() => setSelected(null)}>Close</button><button className="primary-button" onClick={() => toggleArchive(selected.symbol)}>{selected.archived ? "Remove from archive" : "Add to archive"}</button></footer>
      </section></div>}
    </main>
  );
}
