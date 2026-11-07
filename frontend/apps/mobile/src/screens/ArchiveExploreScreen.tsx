import React, { useEffect, useMemo, useState } from "react";
import { Linking, Modal, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import Svg, { Circle, Line, Polygon, Polyline, Rect, Text as SvgText } from "react-native-svg";
import { SafeAreaView } from "react-native-safe-area-context";
import { getInstrumentVector, getInstrumentVectorHistory, getQoinEvents, getQoinCalendar, getQoinInstruments, getQoinNews, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent } from "@qoin/core";

type Market = QoinInstrument["asset_class"];
type Instrument = {
  symbol: string;
  engineSymbol: string;
  name: string;
  market: Market;
  detail: string;
  venue: string;
  provider: string;
  providerStatus?: string;
  archived: boolean;
  tint: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", engineSymbol: "BTCUSDT", name: "Bitcoin", market: "Crypto", detail: "Bitcoin perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, tint: "#E6A23C" },
  { symbol: "ETH/USD", engineSymbol: "ETHUSDT", name: "Ethereum", market: "Crypto", detail: "Ether perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, tint: "#6477DF" },
  { symbol: "SOL/USD", engineSymbol: "SOLUSDT", name: "Solana", market: "Crypto", detail: "Solana perpetual · USDT quote", venue: "Binance USD-M", provider: "Binance", archived: true, tint: "#8B72E8" },
  { symbol: "NVDA", engineSymbol: "NVDA", name: "NVIDIA", market: "Equities", detail: "Semiconductors · NASDAQ", venue: "NASDAQ", provider: "Finnhub", archived: false, tint: "#76A85A" },
  { symbol: "EUR/USD", engineSymbol: "OANDA:EUR_USD", name: "Euro / US Dollar", market: "FX", detail: "Major FX pair", venue: "OANDA", provider: "Finnhub", archived: false, tint: "#4D91D8" },
  { symbol: "AAPL", engineSymbol: "AAPL", name: "Apple", market: "Equities", detail: "Consumer technology · NASDAQ", venue: "NASDAQ", provider: "Finnhub", archived: false, tint: "#8895A7" },
];

type Props = { mode: "Archive" | "Explore" };
const QOIN_FEATURE_SHORT_NAMES = ["OFI", "FUND", "DEPTH", "VOL", "EVENT", "LIQ"] as const;

function FeatureSpace({ decision }: { decision: QoinVectorDecision | null }) {
  const labels = ["OFI", "FUND", "DEPTH", "VOL", "EVENT", "LIQ"];
  const values = QOIN_FEATURE_NAMES.map((name) => Math.max(-1, Math.min(1, decision?.features?.[name] ?? 0)));
  const coordinates = values.map((value, index) => {
    const angle = -Math.PI / 2 + index * Math.PI * 2 / 6;
    const radius = 18 + value * 14;
    return (50 + Math.cos(angle) * radius) + "," + (50 + Math.sin(angle) * radius);
  }).join(" ");
  return <View style={styles.featureSpacePanel}>
    <Text style={styles.attentionTitle}>Six-dimensional feature space</Text>
    <Text style={styles.attentionCaption}>Normalized engine inputs · not a learned embedding</Text>
    <Svg viewBox="0 0 100 100" width="100%" height={190}>
      {[14,24,34].map((radius) => <Polygon key={radius} points={labels.map((_,i) => { const a=-Math.PI/2+i*Math.PI*2/6; return (50+Math.cos(a)*radius)+","+(50+Math.sin(a)*radius); }).join(" ")} fill="none" stroke="#E5EBF4" strokeWidth=".5" />)}
      {labels.map((label,i) => { const a=-Math.PI/2+i*Math.PI*2/6; return <React.Fragment key={label}><Line x1="50" y1="50" x2={50+Math.cos(a)*35} y2={50+Math.sin(a)*35} stroke="#E5EBF4" strokeWidth=".5" /><SvgText x={50+Math.cos(a)*43} y={50+Math.sin(a)*43} fill="#7F8DA1" fontSize="4.5" textAnchor="middle">{label}</SvgText></React.Fragment>; })}
      {decision ? <Polygon points={coordinates} fill="#3475E8" fillOpacity=".16" stroke="#2867DC" strokeWidth="1.2" /> : <SvgText x="50" y="52" fill="#94A3B8" fontSize="5" textAnchor="middle">NO FEATURE VECTOR</SvgText>}
    </Svg>
    <View style={styles.featureValues}>{QOIN_FEATURE_NAMES.map((name,index) => <View key={name} style={styles.featureValue}><Text style={styles.metricLabel}>{labels[index]}</Text><Text style={styles.metricValue}>{values[index].toFixed(3)}</Text></View>)}</View>
  </View>;
}

export function ArchiveExploreScreen({ mode }: Props) {
  const [items, setItems] = useState(seed);
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [vectorLoading, setVectorLoading] = useState(false);
  const [vectorError, setVectorError] = useState("");
  const [vectorHistory, setVectorHistory] = useState<QoinVectorHistory[]>([]);
  const [rawEvents, setRawEvents] = useState<QoinMarketEvent[]>([]);
  const [showRawEvents, setShowRawEvents] = useState(false);
  const [liveBySymbol, setLiveBySymbol] = useState<Record<string, { state?: QoinMarketState; decision?: QoinVectorDecision; event?: QoinStreamUpdate["event"] }>>({});
  const [marketState, setMarketState] = useState<QoinMarketState | null>(null);
  const [streamStatus, setStreamStatus] = useState("disconnected");
  const [lastEvent, setLastEvent] = useState<QoinStreamUpdate["event"] | null>(null);
  const [news, setNews] = useState<QoinNewsItem[]>([]);
  const [newsStatus, setNewsStatus] = useState("loading");
  const [newsCategory, setNewsCategory] = useState("general");
  const [macroEvents, setMacroEvents] = useState<QoinMacroEvent[]>([]);
  const [calendarStatus, setCalendarStatus] = useState("loading");
  const archived = useMemo(() => items.filter((item) => item.archived), [items]);
  const discovered = useMemo(() => items.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [items, market, query]);
  const toggle = (symbol: string) => {
    setItems((current) => current.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
  };
  useEffect(() => {
    let active = true;
    getQoinInstruments().then(({ items }) => {
      if (!active) return;
      const colors = ["#E6A23C", "#6477DF", "#8B72E8", "#4D91D8", "#76A85A", "#8895A7"];
      setItems(items.map((item, index) => ({
        symbol: item.symbol, engineSymbol: item.engine_symbol, name: item.name,
        market: item.asset_class, detail: item.description, venue: item.venue, provider: item.provider, providerStatus: item.provider_status,
        archived: ["BTC/USD", "ETH/USD", "SOL/USD"].includes(item.symbol), tint: colors[index % colors.length],
      })));
    }).catch(() => undefined);
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
  const data = mode === "Archive" ? archived : discovered;
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
    getInstrumentVectorHistory(selected.engineSymbol, 120).then((result) => { if (active) setVectorHistory(result.items); }).catch(() => undefined);
    getQoinEvents(selected.engineSymbol, 200).then((result) => { if (active) setRawEvents(result.items); }).catch(() => undefined);
    return () => { active = false; };
  }, [selected?.engineSymbol]);

  useEffect(() => {
    const symbols = items.map((item) => item.engineSymbol);
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
            setRawEvents((current) => [{
              ...update.event,
              event_id: update.event?.event_id || String(update.server_ts_ms || Date.now()),
              venue: update.event?.venue || "stream", symbol,
              event_type: update.event?.event_type || "unknown",
              ts_ms: update.event?.ts_ms || update.server_ts_ms || Date.now(),
            } as QoinMarketEvent, ...current].slice(0, 200));
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
  }, [items, selected?.engineSymbol]);

  useEffect(() => {
    const current = selected ? liveBySymbol[selected.engineSymbol.toUpperCase()] : undefined;
    setMarketState(current?.state || null);
    setLastEvent(current?.event || null);
  }, [selected?.engineSymbol, liveBySymbol]);

  const bullish = vector?.decision_vector?.[0] ?? 0;
  const bearish = vector?.decision_vector?.[1] ?? 0;
  const vectorPoints = [...vectorHistory, ...(vector?.decision_vector ? [{ decision_vector: vector.decision_vector } as QoinVectorHistory] : [])];
  const vectorScale = Math.max(0.35, ...vectorPoints.flatMap((item) => item.decision_vector));
  const trajectoryPoints = vectorPoints.map((item) => (10 + item.decision_vector[0] / vectorScale * 80) + "," + (90 - item.decision_vector[1] / vectorScale * 80)).join(" ");
  const vectorX = 10 + (bullish / vectorScale) * 80;
  const vectorY = 90 - (bearish / vectorScale) * 80;

  return (
    <SafeAreaView style={styles.screen} edges={["top"]}>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        <View style={styles.topline}><Text style={styles.brand}>qoin</Text><View style={styles.avatar}><Text style={styles.avatarText}>Q</Text></View></View>
        <View style={styles.hero}><Text style={styles.kicker}>YOUR MARKET WORKSPACE</Text><Text style={styles.heroTitle}>See the market{"\n"}<Text style={styles.blue}>as a system.</Text></Text><Text style={styles.heroCopy}>Keep the instruments that matter close. Explore the forces shaping each one.</Text></View>
        <View style={styles.headingRow}><View><Text style={styles.kicker}>{mode === "Archive" ? "YOUR COLLECTION" : "DISCOVERY"}</Text><Text style={styles.heading}>{mode}</Text><Text style={styles.subheading}>{mode === "Archive" ? "Every instrument you follow, in one place." : "Find instruments to bring into your archive."}</Text></View><Text style={styles.count}>{mode === "Archive" ? archived.length : items.length - archived.length} ASSETS</Text></View>

        {mode === "Explore" ? (
          <>
            <View style={styles.search}><Text style={styles.searchIcon}>⌕</Text><TextInput value={query} onChangeText={setQuery} placeholder="Search instruments or symbols" placeholderTextColor="#A4AFBD" style={styles.searchInput} /></View>
            <View style={styles.filters}>{["All", "Crypto", "Equities", "FX", "Indices", "Commodities"].map((name) => <Pressable key={name} onPress={() => setMarket(name)} style={[styles.filter, market === name && styles.filterActive]}><Text style={[styles.filterText, market === name && styles.filterTextActive]}>{name}</Text></Pressable>)}</View>
          </>
        ) : null}

        {data.length ? data.map((item) => (
          <View key={item.symbol} style={styles.card}>
            <Pressable onPress={() => setSelected(item)} style={styles.cardMain}>
              <View style={[styles.assetMark, { backgroundColor: item.tint }]}><Text style={styles.assetMarkText}>{item.symbol.slice(0, 1)}</Text></View>
              <View style={styles.assetCopy}><Text style={styles.assetName}>{item.name}</Text><Text style={styles.assetMeta}>{item.symbol} · {item.market}</Text></View>
              <Text style={styles.arrow}>↗</Text>
            </Pressable>
            <Text style={styles.detail}>{item.detail}</Text><Text style={styles.providerLine}>{item.venue} · {item.provider} · {item.providerStatus || "provider status unknown"}</Text><View style={styles.cardLiveRow}><Text style={styles.cardLivePrice}>{liveBySymbol[item.engineSymbol.toUpperCase()]?.state?.last_price?.toLocaleString(undefined,{maximumFractionDigits:8}) ?? "—"}</Text><Text style={styles.cardLiveDecision}>{liveBySymbol[item.engineSymbol.toUpperCase()]?.decision?.action?.toUpperCase() || "NO LIVE DATA"}</Text></View>
            {mode === "Explore" ? <Pressable onPress={() => toggle(item.symbol)} style={styles.addButton}><Text style={styles.addText}>＋ Add to archive</Text></Pressable> : null}
          </View>
        )) : <View style={styles.empty}><Text style={styles.emptyTitle}>Nothing here yet.</Text><Text style={styles.emptyCopy}>{mode === "Archive" ? "Explore instruments and add the ones you want to follow." : "Try another name or market."}</Text></View>}

        {mode === "Explore" ? <View style={styles.newsSection}>
          <Text style={styles.kicker}>GLOBAL FINANCIAL INTELLIGENCE</Text><Text style={styles.newsTitle}>News & macro events</Text>
          <Text style={styles.noteCopy}>Provider-attributed headlines with publication timestamps and original links.</Text>
          <View style={styles.filters}>{["general", "forex", "crypto", "merger"].map((category) => <Pressable key={category} onPress={() => setNewsCategory(category)} style={[styles.filter, newsCategory === category && styles.filterActive]}><Text style={[styles.filterText, newsCategory === category && styles.filterTextActive]}>{category}</Text></Pressable>)}</View>
          {news.length ? news.slice(0, 25).map((item) => <Pressable key={String(item.id)} style={styles.newsItem} onPress={() => Linking.openURL(item.url).catch(() => undefined)}>
            <Text style={styles.newsMeta}>{item.source} · {item.published_at_ms ? new Date(item.published_at_ms).toLocaleString() : "Timestamp unavailable"}</Text>
            <Text style={styles.newsHeadline}>{item.headline}</Text><Text style={styles.newsSummary} numberOfLines={3}>{item.summary}</Text>
          </Pressable>) : <Text style={styles.noteCopy}>{newsStatus === "loading" ? "Loading sourced headlines…" : newsStatus + ". Configure FINNHUB_API_KEY on the backend. No placeholder headlines are generated."}</Text>}
        </View> : null}
        {mode === "Explore" ? <View style={styles.macroSection}>
          <Text style={styles.kicker}>UPCOMING CATALYSTS</Text><Text style={styles.newsTitle}>Economic calendar</Text>
          <Text style={styles.noteCopy}>Scheduled releases, with actual, estimate and previous values where published.</Text>
          {macroEvents.length ? macroEvents.slice(0,40).map((item,index) => <View key={item.time + item.country + item.event + index} style={styles.macroItem}>
            <Text style={styles.newsHeadline}>{item.event}</Text><Text style={styles.newsMeta}>{item.country} · {item.time} · {item.impact || "Impact not supplied"}</Text>
            <View style={styles.macroValues}><Text style={styles.newsMeta}>Actual: {item.actual ?? "—"}</Text><Text style={styles.newsMeta}>Estimate: {item.estimate ?? "—"}</Text><Text style={styles.newsMeta}>Previous: {item.previous ?? "—"}</Text></View>
          </View>) : <Text style={styles.noteCopy}>{calendarStatus === "loading" ? "Loading economic calendar…" : calendarStatus + ". Configure FINNHUB_API_KEY to load macro events. No event dates are fabricated."}</Text>}
        </View> : null}

        <Text style={styles.footer}>QOIN · RESEARCH MODE · NO LIVE EXECUTION</Text>
      </ScrollView>

      <Modal visible={!!selected} transparent animationType="slide" onRequestClose={() => setSelected(null)}>
        {selected ? <View style={styles.modalShade}><View style={styles.modalCard}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>INSTRUMENT SPACE</Text><Text style={styles.modalTitle}>{selected.name}</Text><Text style={styles.assetMeta}>{selected.symbol} · {selected.market}</Text></View><Pressable onPress={() => setSelected(null)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View>
          <ScrollView contentContainerStyle={styles.modalContent}>
            <Text style={styles.mapTitle}>Force map</Text><Text style={styles.mapCopy}>Persisted Axum decision vector on bullish and bearish axes.</Text>
            <View style={styles.mapWrap}>
              <Svg viewBox="0 0 100 100" width="100%" height={270}>
                <Rect x="0" y="0" width="100" height="100" fill="#FBFCFF" />
                {Array.from({ length: 9 }, (_, i) => <Line key={`v${i}`} x1={i * 12.5} y1="0" x2={i * 12.5} y2="100" stroke="#E9EEF6" strokeWidth=".4" />)}
                {Array.from({ length: 9 }, (_, i) => <Line key={`h${i}`} x1="0" y1={i * 12.5} x2="100" y2={i * 12.5} stroke="#E9EEF6" strokeWidth=".4" />)}
                <Line x1="10" y1="8" x2="10" y2="90" stroke="#CBD5E1" strokeWidth=".7" /><Line x1="10" y1="90" x2="94" y2="90" stroke="#CBD5E1" strokeWidth=".7" />
                {vectorPoints.length > 1 ? <Polyline points={trajectoryPoints} fill="none" stroke="#9BB9F3" strokeWidth="1.1" /> : null}
                {vector?.decision_vector ? <><Line x1="10" y1="90" x2={vectorX} y2={vectorY} stroke="#2867DC" strokeWidth="1.8" /><Circle cx={vectorX} cy={vectorY} r="3.8" fill="#2867DC" stroke="#FFFFFF" strokeWidth="1.2" /></> : null}
                <Circle cx="10" cy="90" r="2" fill="#94A3B8" />
              </Svg>
            </View>
            <Text style={styles.mapCopy}>Horizontal: bullish · Vertical: bearish · {vectorHistory.length} stored observations</Text>
            <View style={styles.livePanel}>
              <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Live market state</Text><Text style={styles.attentionCaption}>{streamStatus}</Text></View>
              <View style={styles.liveGrid}>
                {[
                  ["Last price", marketState?.last_price?.toString() ?? "—"],
                  ["Spread (bps)", marketState?.spread_bps?.toFixed(3) ?? "—"],
                  ["Order flow", marketState?.order_flow_imbalance?.toFixed(4) ?? "—"],
                  ["Funding", marketState?.funding_rate?.toFixed(6) ?? "—"],
                  ["Open interest", marketState?.open_interest?.toString() ?? "—"],
                  ["Liquidations", marketState?.liquidation_events?.toString() ?? "—"],
                  ["Events", marketState?.event_count?.toString() ?? "—"],
                  ["State quality", marketState?.state_quality?.toFixed(3) ?? "—"],
                ].map(([label,value]) => <View key={label} style={styles.liveMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
              </View>
              <Text style={styles.disclaimer}>Last raw event: {lastEvent ? JSON.stringify(lastEvent) : "none received"}. Missing values mean the provider has not supplied that field.</Text>
            </View>
            <FeatureSpace decision={vector} />
            {vectorLoading ? <Text style={styles.disclaimer}>Loading the latest persisted engine vector…</Text> : vector ? (
              <>
                <View style={styles.engineMetrics}>
                  <View><Text style={styles.engineLabel}>Decision</Text><Text style={styles.engineValue}>{vector.action?.toUpperCase() || "WAIT"}</Text></View>
                  <View><Text style={styles.engineLabel}>Magnitude</Text><Text style={styles.engineValue}>{vector.magnitude?.toFixed(3) ?? "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Angle</Text><Text style={styles.engineValue}>{vector.angle_radians?.toFixed(3) ?? "—"} rad</Text></View>
                  <View><Text style={styles.engineLabel}>Strategy</Text><Text style={styles.engineValue}>{vector.strategy_version || "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Bullish component</Text><Text style={styles.engineValue}>{vector.decision_vector?.[0].toFixed(3) ?? "—"}</Text></View>
                  <View><Text style={styles.engineLabel}>Bearish component</Text><Text style={styles.engineValue}>{vector.decision_vector?.[1].toFixed(3) ?? "—"}</Text></View>
                </View>
                {QOIN_FEATURE_NAMES.map((name, index) => {
                  const raw = vector.features?.[name];
                  const attended = vector.attended_features?.[index];
                  const weights = vector.attention?.[index] ?? [];
                  const strongest = weights.length ? weights.indexOf(Math.max(...weights)) : -1;
                  return <View key={name} style={styles.metric}>
                    <Text style={styles.metricLabel}>{name.replaceAll("_", " ")}</Text>
                    <Text style={styles.metricValue}>{raw?.toFixed(3) ?? "—"} → {attended?.toFixed(3) ?? "—"}</Text>
                    {strongest >= 0 ? <Text style={styles.attentionHint}>Strongest attention: {QOIN_FEATURE_NAMES[strongest]} ({weights[strongest].toFixed(2)})</Text> : null}
                  </View>;
                })}
                <View style={styles.attentionPanel}>
                  <View style={styles.attentionHeader}><Text style={styles.attentionTitle}>Attention matrix</Text><Text style={styles.attentionCaption}>Row → column weights</Text></View>
                  <View style={styles.attentionRow}>
                    <Text style={styles.attentionRowLabel} />
                    {QOIN_FEATURE_SHORT_NAMES.map((name) => <Text key={`head-${name}`} style={styles.attentionAxis}>{name}</Text>)}
                  </View>
                  {QOIN_FEATURE_NAMES.map((rowName, rowIndex) => (
                    <View key={rowName} style={styles.attentionRow}>
                      <Text style={styles.attentionRowLabel}>{QOIN_FEATURE_SHORT_NAMES[rowIndex]}</Text>
                      {QOIN_FEATURE_NAMES.map((columnName, columnIndex) => {
                        const weight = vector.attention?.[rowIndex]?.[columnIndex] ?? 0;
                        return <View key={columnName} style={[styles.attentionCell, { backgroundColor: `rgba(40,103,220,${0.08 + weight * 0.82})` }]}>
                          <Text style={[styles.attentionValue, { color: weight > 0.55 ? "#FFFFFF" : "#2B4160" }]}>{weight.toFixed(2)}</Text>
                        </View>;
                      })}
                    </View>
                  ))}
                </View>
                <Text style={styles.disclaimer}>{vector.reason || "Engine research output. Not an order or a profitability claim."}</Text>
              </>
            ) : <Text style={styles.disclaimer}>{vectorError || "No persisted engine vector is available for this symbol yet."}</Text>}
            <View style={styles.rawTape}>
              <Pressable style={styles.rawTapeToggle} onPress={() => setShowRawEvents((value) => !value)}>
                <Text style={styles.attentionTitle}>Raw event tape · {rawEvents.length} observations</Text><Text style={styles.attentionCaption}>{showRawEvents ? "Hide" : "Show"}</Text>
              </Pressable>
              {showRawEvents ? rawEvents.slice(-100).reverse().map((event) => <View key={event.event_id} style={styles.rawEvent}>
                <Text style={styles.newsMeta}>{event.event_type.toUpperCase()} · {new Date(event.ts_ms).toLocaleTimeString()} · {event.venue}</Text>
                <Text style={styles.newsHeadline}>{JSON.stringify(event)}</Text>
              </View>) : null}
            </View>
            <Text style={styles.disclaimer}>The plotted arrow is the persisted research decision vector. Feature inputs and attention links are shown above. This is not an order or a profitability claim.</Text>
          </ScrollView>
          <View style={styles.modalActions}><Pressable style={styles.closeAction} onPress={() => setSelected(null)}><Text style={styles.closeActionText}>Close</Text></Pressable><Pressable style={styles.saveAction} onPress={() => toggle(selected.symbol)}><Text style={styles.saveActionText}>{selected.archived ? "Remove from archive" : "Add to archive"}</Text></Pressable></View>
        </View></View> : null}
      </Modal>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: "#F7F9FC" },
  content: { paddingHorizontal: 17, paddingBottom: 25 },
  topline: { height: 42, flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  brand: { color: "#1B2C45", fontSize: 21, fontWeight: "900", letterSpacing: -1 },
  avatar: { width: 29, height: 29, borderRadius: 15, backgroundColor: "#E8EFFF", alignItems: "center", justifyContent: "center" },
  avatarText: { color: "#2867DC", fontWeight: "800" },
  hero: { paddingTop: 24, paddingBottom: 25 },
  kicker: { color: "#2867DC", fontSize: 9, fontWeight: "800", letterSpacing: 1.5, marginBottom: 7 },
  heroTitle: { color: "#1B2C45", fontSize: 42, lineHeight: 46, fontWeight: "800", letterSpacing: -1.7 },
  blue: { color: "#3475E8" },
  heroCopy: { color: "#8794A7", fontSize: 12, lineHeight: 18, marginTop: 10, maxWidth: 300 },
  headingRow: { flexDirection: "row", alignItems: "flex-end", justifyContent: "space-between", marginBottom: 16 },
  heading: { color: "#1D2E47", fontSize: 24, fontWeight: "800", letterSpacing: -.5 },
  subheading: { color: "#8995A7", fontSize: 10, marginTop: 4 },
  count: { color: "#9AA5B4", fontSize: 8, fontWeight: "800", letterSpacing: 1 },
  search: { height: 42, flexDirection: "row", alignItems: "center", borderWidth: 1, borderColor: "#E5EAF2", borderRadius: 11, backgroundColor: "#FFFFFF", paddingHorizontal: 12, marginBottom: 12 },
  searchIcon: { color: "#8E9BAD", fontSize: 21, marginRight: 8 },
  searchInput: { flex: 1, color: "#24354D", fontSize: 12, paddingVertical: 0 },
  filters: { flexDirection: "row", flexWrap: "wrap", gap: 7, marginBottom: 14 },
  filter: { borderWidth: 1, borderColor: "#E7ECF3", borderRadius: 18, paddingVertical: 6, paddingHorizontal: 11, backgroundColor: "#FFFFFF" },
  filterActive: { borderColor: "#D7E4FF", backgroundColor: "#EDF3FF" },
  filterText: { color: "#8290A3", fontSize: 10, fontWeight: "600" },
  filterTextActive: { color: "#2867D8" },
  card: { padding: 14, backgroundColor: "#FFFFFF", borderWidth: 1, borderColor: "#E8EDF4", borderRadius: 14, marginBottom: 9 },
  cardMain: { flexDirection: "row", alignItems: "center" },
  assetMark: { width: 37, height: 37, borderRadius: 12, alignItems: "center", justifyContent: "center" },
  assetMarkText: { color: "#FFFFFF", fontWeight: "800", fontSize: 15 },
  assetCopy: { flex: 1, marginLeft: 11 },
  assetName: { color: "#20324B", fontSize: 13, fontWeight: "700" },
  assetMeta: { color: "#929EAF", fontSize: 10, marginTop: 4 },
  arrow: { color: "#A5B1C1", fontSize: 17 },
  detail: { color: "#8B98AA", fontSize: 10, marginTop: 12 },
  providerLine: { color: "#A0AABA", fontSize: 8, marginTop: 4 },
  cardLiveRow: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginTop: 8 },
  cardLivePrice: { color: "#2B4160", fontSize: 12, fontWeight: "700", fontVariant: ["tabular-nums"] },
  cardLiveDecision: { color: "#2867D8", fontSize: 8, fontWeight: "800" },
  addButton: { marginTop: 12, alignSelf: "flex-start", paddingVertical: 5 },
  addText: { color: "#2867D8", fontSize: 10, fontWeight: "700" },
  empty: { padding: 28, borderWidth: 1, borderStyle: "dashed", borderColor: "#DCE4EF", borderRadius: 14, alignItems: "center" },
  emptyTitle: { color: "#34465F", fontSize: 13, fontWeight: "700" },
  emptyCopy: { color: "#8996A8", fontSize: 10, textAlign: "center", marginTop: 6 },
  note: { marginTop: 15, padding: 14, borderWidth: 1, borderColor: "#E8EDF4", borderRadius: 12, backgroundColor: "#FBFCFE" },
  newsSection: { marginTop: 23, paddingTop: 18, borderTopWidth: 1, borderTopColor: "#E9EDF4" },
  macroSection: { marginTop: 20, paddingTop: 17, borderTopWidth: 1, borderTopColor: "#E9EDF4" },
  macroItem: { padding: 11, borderWidth: 1, borderColor: "#E8EDF4", borderRadius: 10, backgroundColor: "#FFFFFF", marginBottom: 7 },
  macroValues: { flexDirection: "row", flexWrap: "wrap", justifyContent: "space-between", gap: 8, marginTop: 7 },
  newsTitle: { color: "#1A2A42", fontSize: 19, fontWeight: "800", marginBottom: 5 },
  newsItem: { padding: 12, backgroundColor: "#FFFFFF", borderWidth: 1, borderColor: "#E8EDF4", borderRadius: 11, marginBottom: 8 },
  newsMeta: { color: "#9AA6B5", fontSize: 8 },
  newsHeadline: { color: "#2A4161", fontSize: 12, fontWeight: "700", lineHeight: 17, marginTop: 7 },
  newsSummary: { color: "#8290A3", fontSize: 10, lineHeight: 15, marginTop: 5 },
  noteTitle: { color: "#4C5D75", fontSize: 11, fontWeight: "700" },
  noteCopy: { color: "#8A97A9", fontSize: 10, lineHeight: 15, marginTop: 5 },
  footer: { color: "#A1ABBA", fontSize: 8, fontWeight: "700", letterSpacing: 1, textAlign: "center", marginTop: 25 },
  modalShade: { flex: 1, justifyContent: "flex-end", backgroundColor: "#17294366" },
  modalCard: { maxHeight: "94%", backgroundColor: "#FFFFFF", borderTopLeftRadius: 21, borderTopRightRadius: 21, overflow: "hidden" },
  modalHeader: { flexDirection: "row", justifyContent: "space-between", alignItems: "flex-start", padding: 19, borderBottomWidth: 1, borderBottomColor: "#EDF1F6" },
  modalTitle: { color: "#1D2E47", fontSize: 22, fontWeight: "800" },
  close: { width: 31, height: 31, borderRadius: 16, backgroundColor: "#F1F4F8", alignItems: "center", justifyContent: "center" },
  closeText: { color: "#65758B", fontSize: 20 },
  modalContent: { paddingHorizontal: 18, paddingTop: 15, paddingBottom: 18 },
  mapTitle: { color: "#34465E", fontSize: 12, fontWeight: "700" },
  mapCopy: { color: "#8B98A9", fontSize: 10, marginTop: 4 },
  mapWrap: { width: "100%", marginTop: 10, borderWidth: 1, borderColor: "#EDF1F6", borderRadius: 12, overflow: "hidden" },
  legend: { flexDirection: "row", flexWrap: "wrap", justifyContent: "center", gap: 12, marginVertical: 14 },
  legendItem: { flexDirection: "row", alignItems: "center", gap: 5 },
  legendDot: { width: 7, height: 7, borderRadius: 4 },
  legendText: { color: "#7D8A9C", fontSize: 9 },
  metric: { flexDirection: "row", justifyContent: "space-between", paddingVertical: 9, borderBottomWidth: 1, borderBottomColor: "#EDF1F6" },
  metricLabel: { color: "#8290A3", fontSize: 10, textTransform: "capitalize" },
  metricValue: { color: "#40536E", fontSize: 10, fontWeight: "700" },
  engineMetrics: { flexDirection: "row", flexWrap: "wrap", gap: 9, marginVertical: 12 },
  engineLabel: { color: "#8996A8", fontSize: 8, textTransform: "uppercase", letterSpacing: .5 },
  engineValue: { color: "#2B4160", fontSize: 12, fontWeight: "700", marginTop: 4 },
  attentionHint: { color: "#9AA6B5", fontSize: 8, marginTop: 5 },
  attentionPanel: { marginTop: 14, padding: 10, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, backgroundColor: "#FBFCFE" },
  livePanel: { marginTop: 12, padding: 10, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, backgroundColor: "#FBFCFE" },
  liveHeading: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginBottom: 9 },
  liveGrid: { flexDirection: "row", flexWrap: "wrap", gap: 10 },
  liveMetric: { width: "47%", gap: 4 },
  featureSpacePanel: { marginTop: 14, padding: 10, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, backgroundColor: "#FBFCFE" },
  rawTape: { marginTop: 14, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, overflow: "hidden" },
  rawTapeToggle: { padding: 11, flexDirection: "row", justifyContent: "space-between", alignItems: "center", backgroundColor: "#FBFCFE" },
  rawEvent: { padding: 9, borderTopWidth: 1, borderTopColor: "#EDF1F6" },
  featureValues: { flexDirection: "row", flexWrap: "wrap", justifyContent: "space-between", gap: 7 },
  featureValue: { width: "30%", flexDirection: "row", justifyContent: "space-between" },
  attentionHeader: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 9 },
  attentionTitle: { color: "#40536E", fontSize: 10, fontWeight: "700" },
  attentionCaption: { color: "#9AA6B5", fontSize: 8 },
  attentionRow: { flexDirection: "row", alignItems: "center", gap: 3, marginBottom: 3 },
  attentionRowLabel: { width: 36, color: "#8794A7", fontSize: 8, fontWeight: "700" },
  attentionAxis: { flex: 1, color: "#8794A7", fontSize: 7, fontWeight: "700", textAlign: "center" },
  attentionCell: { flex: 1, minWidth: 0, height: 25, borderRadius: 4, alignItems: "center", justifyContent: "center" },
  attentionValue: { fontSize: 8, fontVariant: ["tabular-nums"] },
  disclaimer: { color: "#8996A7", fontSize: 9, lineHeight: 14, padding: 11, backgroundColor: "#F7F9FC", borderRadius: 9, marginTop: 15 },
  modalActions: { flexDirection: "row", justifyContent: "flex-end", gap: 8, padding: 14, borderTopWidth: 1, borderTopColor: "#EDF1F6" },
  closeAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#F0F3F8" },
  closeActionText: { color: "#64748B", fontSize: 11, fontWeight: "700" },
  saveAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#2867DC" },
  saveActionText: { color: "#FFFFFF", fontSize: 11, fontWeight: "700" },
});
