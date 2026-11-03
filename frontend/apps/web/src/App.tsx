import { useEffect, useMemo, useState, type FormEvent } from "react";
import { getQoinHealth, getQoinPortfolio, saveQoinPortfolio, type QoinHealth } from "@qoin/core";
import { createUserWithEmailAndPassword, onAuthStateChanged, signInWithEmailAndPassword, signOut, type User } from "firebase/auth";
import { firebaseAuth, isFirebaseConfigured } from "./firebaseAuth";
import { WATCHLIST } from "@qoin/core";

type PrimaryTab = "archive" | "feed";
type AssetClass = "All" | "Forex" | "Crypto" | "Stocks" | "Options";

const PORTFOLIO_STORAGE_KEY = "qoin.portfolio.v1";

function readSavedBasket(): string[] {
  if (typeof window === "undefined") return [];
  try {
    const stored: unknown = JSON.parse(window.localStorage.getItem(PORTFOLIO_STORAGE_KEY) ?? "[]");
    if (!Array.isArray(stored)) return [];
    const allowed = new Set<string>(WATCHLIST);
    return [...new Set(stored.filter((item): item is string => typeof item === "string" && allowed.has(item)))];
  } catch {
    return [];
  }
}

const assets = WATCHLIST.map((symbol) => ({
  symbol,
  name: symbol.split("/")[0] === "BTC" ? "Bitcoin" :
    symbol.split("/")[0] === "ETH" ? "Ethereum" :
    symbol.split("/")[0] === "SOL" ? "Solana" :
    symbol.split("/")[0] === "AVAX" ? "Avalanche" : symbol,
  category: "Crypto" as const,
  venue: "Digital assets",
}));

function App() {
  const [tab, setTab] = useState<PrimaryTab>("archive");
  const [filter, setFilter] = useState<AssetClass>("All");
  const [selectedAsset, setSelectedAsset] = useState<string | null>(null);
  const [portfolioOpen, setPortfolioOpen] = useState(false);
  const [basket, setBasket] = useState<string[]>(readSavedBasket);
  const [health, setHealth] = useState<QoinHealth | null>(null);
  const [authUser, setAuthUser] = useState<User | null>(firebaseAuth?.currentUser ?? null);
  const [serverBasketOwnerUid, setServerBasketOwnerUid] = useState<string | null>(null);
  const [authOpen, setAuthOpen] = useState(false);
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [creatingAccount, setCreatingAccount] = useState(false);
  const [authBusy, setAuthBusy] = useState(false);
  const [authError, setAuthError] = useState<string | null>(null);

  useEffect(() => {
    getQoinHealth().then(setHealth).catch(() => setHealth(null));
  }, []);

  useEffect(() => {
    if (!firebaseAuth) return undefined;
    return onAuthStateChanged(firebaseAuth, setAuthUser);
  }, []);

  useEffect(() => {
    let active = true;
    if (!authUser) {
      setServerBasketOwnerUid(null);
      return () => { active = false; };
    }
    setServerBasketOwnerUid(null);
    void authUser.getIdToken().then(getQoinPortfolio).then((portfolio) => {
      if (!active) return;
      const allowed = new Set<string>(WATCHLIST);
      setBasket([...new Set(portfolio.symbols.filter((symbol) => allowed.has(symbol)))]);
    }).catch(() => {
      // The local basket remains usable when server auth/storage is unavailable.
    }).finally(() => {
      if (active) setServerBasketOwnerUid(authUser.uid);
    });
    return () => { active = false; };
  }, [authUser]);

  useEffect(() => {
    if (!authUser || serverBasketOwnerUid !== authUser.uid) return;
    void authUser.getIdToken().then((token) => saveQoinPortfolio(basket, token)).catch(() => {
      // Local state remains available if the API has not been configured.
    });
  }, [authUser, basket, serverBasketOwnerUid]);

  async function submitAuth(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!firebaseAuth) return;
    setAuthBusy(true);
    setAuthError(null);
    try {
      if (creatingAccount) {
        await createUserWithEmailAndPassword(firebaseAuth, email.trim(), password);
      } else {
        await signInWithEmailAndPassword(firebaseAuth, email.trim(), password);
      }
      setAuthOpen(false);
      setPassword("");
    } catch (reason) {
      setAuthError(reason instanceof Error ? reason.message : "Authentication failed. Please try again.");
    } finally {
      setAuthBusy(false);
    }
  }

  useEffect(() => {
    try {
      window.localStorage.setItem(PORTFOLIO_STORAGE_KEY, JSON.stringify(basket));
    } catch {
      // Storage may be unavailable in private browsing or restricted contexts.
    }
  }, [basket]);

  const visibleAssets = useMemo(
    () => assets.filter((asset) => filter === "All" || asset.category === filter),
    [filter],
  );
  const selected = assets.find((asset) => asset.symbol === selectedAsset);

  function toggleBasket(symbol: string) {
    setBasket((current) =>
      current.includes(symbol) ? current.filter((item) => item !== symbol) : [...current, symbol],
    );
  }

  return (
    <main className="qoin-shell">
      <header className="qoin-topbar">
        <a className="qoin-brand" href="/" aria-label="Qoin home">
          <span className="qoin-brand-mark">Q</span>
          <span>QOIN</span>
        </a>
        <div className="topbar-right">
          <span className="connection-status">
            <span className={health ? "status-dot online" : "status-dot"} />
            {health ? "API reachable" : "API unavailable"}
          </span>
          {authUser ? (
            <button className="account-trigger" onClick={() => { if (firebaseAuth) void signOut(firebaseAuth); }}>Sign out</button>
          ) : isFirebaseConfigured ? (
            <button className="account-trigger" onClick={() => setAuthOpen(true)}>Sign in</button>
          ) : (
            <span className="preview-account">Preview mode</span>
          )}
          <button
            className="portfolio-trigger"
            onClick={() => setPortfolioOpen(true)}
            aria-label="Open my portfolio"
            title="My portfolio"
          >
            <span className="portfolio-trigger-icon">◈</span>
            <span className="portfolio-trigger-label">Portfolio</span>
            <span className="basket-count">{basket.length}</span>
          </button>
        </div>
      </header>

      <div className="qoin-content">
        {selected ? (
          <InstrumentSpace
            asset={selected}
            inBasket={basket.includes(selected.symbol)}
            onBack={() => setSelectedAsset(null)}
            onToggleBasket={() => toggleBasket(selected.symbol)}
          />
        ) : (
          <>
            <section className="workspace-heading">
              <div>
                <p className="eyebrow">YOUR MARKET UNIVERSE</p>
                <h1>Discover. Understand. <span>Compose.</span></h1>
                <p className="workspace-description">
                  Explore instruments, follow market intelligence and build the basket your Qoin algorithms manage.
                </p>
              </div>
            </section>

            <nav className="primary-tabs" aria-label="Main navigation">
              <button className={tab === "archive" ? "primary-tab active" : "primary-tab"} onClick={() => setTab("archive")}>
                <span aria-hidden="true">▤</span> Archive
              </button>
              <button className={tab === "feed" ? "primary-tab active" : "primary-tab"} onClick={() => setTab("feed")}>
                <span aria-hidden="true">◉</span> Feed
              </button>
            </nav>

            {tab === "archive" ? (
              <section className="archive-view">
                <div className="section-heading">
                  <div>
                    <p className="eyebrow">INSTRUMENT LIBRARY</p>
                    <h2>Archive</h2>
                    <p className="section-copy">Your saved and available trading instruments.</p>
                  </div>
                  <span className="result-count">{assets.length} instruments in starter universe</span>
                </div>
                <div className="filter-row" role="group" aria-label="Filter instruments">
                  {(["All", "Forex", "Crypto", "Stocks", "Options"] as AssetClass[]).map((item) => (
                    <button key={item} className={filter === item ? "filter-chip selected" : "filter-chip"} onClick={() => setFilter(item)}>
                      {item}
                    </button>
                  ))}
                </div>
                <div className="instrument-list">
                  {visibleAssets.length ? visibleAssets.map((asset) => (
                    <button className="instrument-row" key={asset.symbol} onClick={() => setSelectedAsset(asset.symbol)}>
                      <span className="asset-mark">{asset.symbol.split("/")[0].slice(0, 1)}</span>
                      <span className="asset-identity">
                        <strong>{asset.name}</strong>
                        <small>{asset.symbol} <span>·</span> {asset.venue}</small>
                      </span>
                      <span className="asset-state"><i /> Vector data pending</span>
                      <span className="row-chevron">›</span>
                    </button>
                  )) : (
                    <div className="empty-state">
                      <span className="empty-symbol">⌁</span>
                      <strong>No instruments in this category yet</strong>
                      <p>The current connected starter universe contains crypto instruments. Additional market universes will appear when their data providers are connected.</p>
                    </div>
                  )}
                </div>
                <p className="data-disclaimer">The current instrument list is the repository's starter watchlist. Market quotes and live factor vectors are not yet supplied by a production market-data provider.</p>
              </section>
            ) : (
              <FeedView onOpenAsset={setSelectedAsset} basket={basket} onToggleBasket={toggleBasket} />
            )}
          </>
        )}
      </div>

      {authOpen && (
        <div className="drawer-backdrop" role="presentation" onClick={() => setAuthOpen(false)}>
          <section className="auth-dialog" role="dialog" aria-modal="true" aria-label="Qoin account" onClick={(event) => event.stopPropagation()}>
            <div className="drawer-heading">
              <div><p className="eyebrow">QOIN IDENTITY</p><h2>{creatingAccount ? "Create account" : "Sign in"}</h2></div>
              <button className="icon-button" onClick={() => setAuthOpen(false)} aria-label="Close sign in">×</button>
            </div>
            <p className="drawer-copy">Your account is authenticated by Qoin's Firebase project. The API verifies your ID token before loading or saving your portfolio.</p>
            <form className="auth-form" onSubmit={(event) => void submitAuth(event)}>
              <label htmlFor="qoin-email">Email address</label>
              <input id="qoin-email" type="email" autoComplete="email" required value={email} onChange={(event) => setEmail(event.target.value)} />
              <label htmlFor="qoin-password">Password</label>
              <input id="qoin-password" type="password" minLength={6} autoComplete={creatingAccount ? "new-password" : "current-password"} required value={password} onChange={(event) => setPassword(event.target.value)} />
              {authError ? <p className="auth-error" role="alert">{authError}</p> : null}
              <button className="primary-action" type="submit" disabled={authBusy}>{authBusy ? "Please wait…" : creatingAccount ? "Create account" : "Sign in"}</button>
            </form>
            <button className="auth-switch" onClick={() => { setCreatingAccount((value) => !value); setAuthError(null); }}>{creatingAccount ? "Already have an account? Sign in" : "New to Qoin? Create an account"}</button>
            <p className="data-disclaimer">Enable Email/Password in Qoin's Firebase Console. Do not reuse credentials from another project.</p>
          </section>
        </div>
      )}

      {portfolioOpen && (
        <div className="drawer-backdrop" role="presentation" onClick={() => setPortfolioOpen(false)}>
          <aside className="portfolio-drawer" role="dialog" aria-modal="true" aria-label="My portfolio" onClick={(event) => event.stopPropagation()}>
            <div className="drawer-heading">
              <div><p className="eyebrow">YOUR ASSET BASKET</p><h2>My portfolio</h2></div>
              <button className="icon-button" onClick={() => setPortfolioOpen(false)} aria-label="Close portfolio">×</button>
            </div>
            <p className="drawer-copy">Choose the instruments you want Qoin to evaluate. {authUser ? "Your basket is synchronized to your Qoin account when the API is configured." : "Your basket is saved on this device until you sign in."} This basket does not place orders.</p>
            {basket.length ? (
              <div className="drawer-assets">
                {basket.map((symbol) => {
                  const asset = assets.find((item) => item.symbol === symbol);
                  return <div className="drawer-asset" key={symbol}>
                    <span className="asset-mark">{symbol.split("/")[0].slice(0, 1)}</span>
                    <span><strong>{asset?.name ?? symbol}</strong><small>{symbol}</small></span>
                    <button className="remove-link" onClick={() => toggleBasket(symbol)}>Remove</button>
                  </div>;
                })}
              </div>
            ) : (
              <div className="empty-state drawer-empty">
                <span className="empty-symbol">◈</span>
                <strong>Your basket is waiting</strong>
                <p>Explore Archive or Feed and add instruments to begin building your portfolio.</p>
              </div>
            )}
            <div className="portfolio-summary">
              <div><span>Selected instruments</span><strong>{basket.length}</strong></div>
              <div><span>Portfolio allocation</span><strong>Algorithm-managed</strong></div>
              <div><span>Execution</span><strong>Not enabled</strong></div>
            </div>
            <p className="data-disclaimer">Portfolio analytics and automated execution remain unavailable until their respective data, risk and broker integrations are connected and authorized.</p>
          </aside>
        </div>
      )}
      <footer className="qoin-footer"><span>QOIN · QUANTITATIVE PORTFOLIO INTELLIGENCE</span><span>RESEARCH MODE · NO LIVE EXECUTION</span></footer>
    </main>
  );
}

function FeedView({ onOpenAsset, basket, onToggleBasket }: {
  onOpenAsset: (symbol: string) => void;
  basket: string[];
  onToggleBasket: (symbol: string) => void;
}) {
  return (
    <section className="feed-view">
      <div className="section-heading">
        <div><p className="eyebrow">MARKET INTELLIGENCE</p><h2>Feed</h2><p className="section-copy">Discover instruments and follow the information shaping markets.</p></div>
      </div>
      <div className="feed-layout">
        <div className="feed-stream">
          <article className="feed-card">
            <div className="feed-card-meta"><span className="feed-avatar">Q</span><span><strong>Qoin Intelligence</strong><small>Discovery desk · Instrument universe</small></span><span className="feed-tag">DISCOVERY</span></div>
            <h3>Build your market universe</h3>
            <p>Explore the available instruments below. Open an instrument to inspect its vector workspace, then add it to your portfolio basket when it matches your investment intent.</p>
            <div className="feed-asset-grid">
              {assets.map((asset) => (
                <div className="feed-asset" key={asset.symbol}>
                  <button className="feed-asset-main" onClick={() => onOpenAsset(asset.symbol)}>
                    <span className="asset-mark">{asset.symbol.split("/")[0].slice(0, 1)}</span>
                    <span><strong>{asset.name}</strong><small>{asset.symbol}</small></span>
                  </button>
                  <button className={basket.includes(asset.symbol) ? "add-asset added" : "add-asset"} onClick={() => onToggleBasket(asset.symbol)}>
                    {basket.includes(asset.symbol) ? "In basket ✓" : "+ Add"}
                  </button>
                </div>
              ))}
            </div>
            <div className="feed-card-footer"><span>Starter universe · Crypto</span><span>Instrument catalogue</span></div>
          </article>
          <article className="feed-card feed-placeholder">
            <div className="feed-card-meta"><span className="feed-avatar muted">◎</span><span><strong>Market news and community</strong><small>Intelligence stream</small></span><span className="feed-tag muted-tag">CONNECTING</span></div>
            <h3>Your financial discovery feed</h3>
            <p>Verified market news, research notes, asset launches and community discussions will appear here as the corresponding feeds are integrated. No placeholder headlines are presented as real market news.</p>
            <div className="feed-card-footer"><span>News provider not connected</span><span>Social feed not connected</span></div>
          </article>
        </div>
        <aside className="feed-side-panel">
          <p className="eyebrow">HOW QOIN WORKS</p>
          <h3>Your intent. The algorithm's work.</h3>
          <ol><li><b>Discover</b><span>Find instruments in Feed or Archive.</span></li><li><b>Compose</b><span>Add assets to your portfolio basket.</span></li><li><b>Evaluate</b><span>Quantitative engines assess factors, relationships and risk.</span></li><li><b>Manage</b><span>Authorized algorithms manage the portfolio within your constraints.</span></li></ol>
        </aside>
      </div>
    </section>
  );
}

function InstrumentSpace({ asset, inBasket, onBack, onToggleBasket }: {
  asset: (typeof assets)[number];
  inBasket: boolean;
  onBack: () => void;
  onToggleBasket: () => void;
}) {
  return (
    <section className="instrument-space">
      <button className="back-link" onClick={onBack}>‹ Back to Archive</button>
      <div className="instrument-title-row">
        <div><p className="eyebrow">INDIVIDUAL VECTOR SPACE</p><h1>{asset.name} <span>{asset.symbol}</span></h1><p className="section-copy">{asset.category} · {asset.venue}</p></div>
        <button className="primary-action" onClick={onToggleBasket}>{inBasket ? "Remove from portfolio" : "+ Add to portfolio"}</button>
      </div>
      <div className="vector-workspace">
        <div className="vector-visual-panel">
          <div className="vector-panel-heading"><div><p className="eyebrow">FACTOR SPACE</p><h2>Instrument vector</h2></div><span className="pending-pill">Awaiting market data</span></div>
          <div className="vector-stage">
            <svg viewBox="0 0 640 390" role="img" aria-label="Instrument vector coordinate space awaiting market data">
              <defs><pattern id="grid" width="32" height="32" patternUnits="userSpaceOnUse"><path d="M 32 0 L 0 0 0 32" fill="none" stroke="#28333f" strokeWidth="1"/></pattern></defs>
              <rect width="640" height="390" fill="url(#grid)"/>
              <path d="M75 315 L565 315 M320 350 L320 35 M112 350 L485 70" stroke="#657586" strokeWidth="1.4" opacity=".8"/>
              <path d="M565 315 l-9 -5 v10 z M320 35 l-5 9 h10 z M485 70 l-10 2 6 8 z" fill="#8b9bad"/>
              <text x="570" y="320" fill="#8b9bad" fontSize="11">Factor 1</text><text x="330" y="34" fill="#8b9bad" fontSize="11">Factor 2</text><text x="490" y="63" fill="#8b9bad" fontSize="11">Factor 3</text>
              <circle cx="320" cy="210" r="5" fill="#b8f36a" opacity=".8"/><circle cx="320" cy="210" r="22" fill="none" stroke="#b8f36a" opacity=".25"/>
              <text x="320" y="250" textAnchor="middle" fill="#9aa8b8" fontSize="12">Vector position will render when measured inputs arrive</text>
            </svg>
          </div>
          <p className="data-disclaimer">This is a coordinate-space scaffold, not a live or simulated instrument position. The canonical vector will be calculated from normalized market factors; the 3D projection is for visualization only.</p>
        </div>
        <div className="factor-panel">
          <p className="eyebrow">VECTOR COMPONENTS</p><h2>Factor measurements</h2>
          {["Return / momentum", "Volatility", "Market beta", "Liquidity / depth", "Order-flow imbalance", "Funding / basis", "Open-interest change"].map((name) => <div className="factor-row" key={name}><span>{name}</span><strong>—</strong></div>)}
          <div className="factor-note">Measurements appear when a verified provider supplies the required observations.</div>
        </div>
      </div>
      <div className="vector-lower-grid">
        <div className="info-panel"><p className="eyebrow">HISTORICAL TRAJECTORY</p><h3>Movement history</h3><p>Historical vector paths are not yet available for this instrument.</p></div>
        <div className="info-panel"><p className="eyebrow">RELATIONSHIPS</p><h3>Cross-instrument relationships</h3><p>Correlation and covariance links will appear after synchronized market observations are available.</p></div>
      </div>
    </section>
  );
}

export default App;
