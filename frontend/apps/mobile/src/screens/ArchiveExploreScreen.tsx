import React, { useEffect, useMemo, useRef, useState } from "react";
import { Alert, Image } from "react-native";
import * as ImagePicker from "expo-image-picker";
import type { User } from "firebase/auth";
import { Linking, Modal, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import Svg, { Circle, Line, Polygon, Polyline, Rect, Text as SvgText } from "react-native-svg";
import { SafeAreaView } from "react-native-safe-area-context";
import { subscribeQoinAuth, signOutQoinUser, uploadQoinAvatar, getInstrumentVector, getInstrumentVectorHistory, getQoinEvents, getQoinCalendar, getQoinInstruments, getQoinNews, getQoinStrategy, getQoinAlgorithmCatalogue, runQoinBacktest, subscribeQoinStream, QOIN_FEATURE_NAMES, type QoinAlgorithmCatalogue, type QoinBacktestResult, type QoinVectorDecision, type QoinVectorHistory, type QoinMarketState, type QoinNewsItem, type QoinInstrument, type QoinStreamUpdate, type QoinMacroEvent, type QoinMarketEvent, type QoinStrategy } from "@qoin/core";

type Market = QoinInstrument["asset_class"];
type Instrument = {
  symbol: string;
  engineSymbol: string;
  name: string;
  market: Market;
  detail: string;
  venue: string;
  engineVenue: string;
  provider: string;
  providerStatus?: string;
  archived: boolean;
  tint: string;
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", engineSymbol: "BTCUSDT", name: "Bitcoin", market: "Crypto", detail: "Bitcoin perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#E6A23C" },
  { symbol: "ETH/USD", engineSymbol: "ETHUSDT", name: "Ethereum", market: "Crypto", detail: "Ether perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#6477DF" },
  { symbol: "SOL/USD", engineSymbol: "SOLUSDT", name: "Solana", market: "Crypto", detail: "Solana perpetual · USDT quote", venue: "Binance USD-M", engineVenue: "binance_usdm", provider: "Binance", archived: true, tint: "#8B72E8" },
  { symbol: "NVDA", engineSymbol: "NVDA", name: "NVIDIA", market: "Equities", detail: "Semiconductors · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#76A85A" },
  { symbol: "EUR/USD", engineSymbol: "OANDA:EUR_USD", name: "Euro / US Dollar", market: "FX", detail: "Major FX pair", venue: "OANDA", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#4D91D8" },
  { symbol: "AAPL", engineSymbol: "AAPL", name: "Apple", market: "Equities", detail: "Consumer technology · NASDAQ", venue: "NASDAQ", engineVenue: "finnhub", provider: "Finnhub", archived: false, tint: "#8895A7" },
];

type Props = { mode: "Archive" | "Explore" };
type QoinNotification = { id: string; symbol: string; title: string; body: string; kind: "signal" | "liquidation"; createdAt: number; read: boolean; details: string };
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


function DecisionTracePanel({ decision }: { decision: QoinVectorDecision }) {
  const gates = decision.diagnostics;
  const rows = decision.feature_trace || [];
  const gateRows: [string, boolean | undefined][] = [
    ["Spread supplied", gates?.spread_present], ["Spread within limit", gates?.spread_within_limit],
    ["Event evidence", gates?.event_intensity_positive], ["Magnitude threshold", gates?.magnitude_passed],
    ["Buy angle", gates?.buy_angle_passed], ["Sell angle", gates?.sell_angle_passed],
    ["Bullish > bearish", gates?.bullish_exceeds_bearish], ["Bearish > bullish", gates?.bearish_exceeds_bullish],
  ];
  return <View style={styles.tracePanel}>
    <Text style={styles.attentionTitle}>Decision trace · nothing hidden</Text>
    <Text style={styles.attentionCaption}>Exact calculations and parameters for {decision.strategy_version || "this decision"}</Text>
    {decision.input_event ? <View style={styles.traceSource}>
      <Text style={styles.engineLabel}>Trigger event · {decision.input_event.event_type} · {decision.input_event.venue}</Text>
      <Text style={styles.traceValue}>{decision.input_event.event_id}</Text>
      <Text style={styles.attentionCaption}>{new Date(decision.input_event.event_ts_ms).toLocaleString()} · {decision.input_event.state_event_count} accumulated events</Text>
    </View> : null}
    <ScrollView horizontal showsHorizontalScrollIndicator>
      <View>
        <View style={styles.traceTableRow}>
          {["Feature","Raw","Transform","Scale","Normalized","Clamped","Bull term","Bear term"].map((label) => <Text key={label} style={[styles.traceCell,styles.traceHead]}>{label}</Text>)}
        </View>
        {rows.map((item) => <View key={item.name} style={styles.traceTableRow}>
          {[item.name.replaceAll("_"," "),item.raw_value.toPrecision(4),item.transform,String(item.scale),item.normalized_value.toFixed(4),item.clamped_value.toFixed(4),item.bullish_weighted_term.toFixed(4),item.bearish_weighted_term.toFixed(4)].map((value,index) => <Text key={index} style={styles.traceCell}>{value}</Text>)}
        </View>)}
      </View>
    </ScrollView>
    <View style={styles.traceGateGrid}>
      {gateRows.map(([label,value]) => <View key={label} style={styles.traceGate}>
        <Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value == null ? "Not reported" : value ? "PASS" : "FAIL"}</Text>
      </View>)}
    </View>
    <Text style={styles.disclaimer}>Angle {decision.angle_degrees?.toFixed(4) ?? "—"}° · angle limit {gates?.angle_limit_degrees ?? "—"}° · magnitude limit {gates?.magnitude_limit ?? "—"} · spread limit {gates?.spread_limit_bps ?? "—"} bps. {decision.reason || ""}</Text>
    <Text style={styles.traceSubhead}>Raw inputs (null means not supplied)</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify({ ...decision.raw_inputs, liquidation_side: decision.liquidation_side }, null, 2)}</Text>
    <Text style={styles.traceSubhead}>Exact strategy parameter snapshot</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify(decision.strategy_parameters || null, null, 2)}</Text>
    <Text style={styles.traceSubhead}>Complete serialized decision</Text>
    <Text selectable style={styles.traceJson}>{JSON.stringify(decision, null, 2)}</Text>
  </View>;
}

function MobileBacktestPanel({ symbol, venue, authenticated, onRequireAuth }: { symbol: string; venue: string; authenticated: boolean; onRequireAuth: () => void }) {
  const [capital, setCapital] = useState("10000");
  const [fee, setFee] = useState("10");
  const [slippage, setSlippage] = useState("5");
  const [result, setResult] = useState<QoinBacktestResult | null>(null);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);
  const run = async () => {
    if (!authenticated) { onRequireAuth(); setStatus("Sign in to run a historical research replay."); return; }
    const initialCapital = Number(capital), feeBps = Number(fee), slippageBps = Number(slippage);
    if (!Number.isFinite(initialCapital) || initialCapital <= 0 || !Number.isFinite(feeBps) || feeBps < 0 ||
        !Number.isFinite(slippageBps) || slippageBps < 0 || slippageBps >= 10_000) {
      setStatus("Enter positive capital, non-negative fees and slippage below 10,000 bps.");
      return;
    }
    setBusy(true); setStatus(""); setResult(null);
    try {
      const replay = await runQoinBacktest({ symbol, venue, initial_capital: initialCapital, fee_bps: feeBps, slippage_bps: slippageBps, limit: 10_000 });
      setResult(replay); setStatus("Replay completed from persisted events only.");
    } catch (error) {
      setStatus(error instanceof Error ? error.message : "Historical replay unavailable.");
    } finally { setBusy(false); }
  };
  return <View style={styles.mobileBacktest}>
    <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Historical event replay</Text><Text style={styles.attentionCaption}>Paper only · no orders</Text></View>
    <Text style={styles.disclaimer}>Uses stored events for {venue}. No missing history is downloaded. This is a research simulation, not expected performance.</Text>
    <View style={styles.backtestInputs}>
      {[["Capital", capital, setCapital], ["Fee (bps)", fee, setFee], ["Slippage (bps)", slippage, setSlippage]].map(([label, value, setter]) => (
        <View key={String(label)} style={styles.backtestInputWrap}>
          <Text style={styles.engineLabel}>{String(label)}</Text>
          <TextInput value={String(value)} onChangeText={setter as (value: string) => void} keyboardType="decimal-pad" style={styles.backtestInput} />
        </View>
      ))}
    </View>
    <Pressable onPress={run} disabled={busy} style={[styles.backtestRun, busy && styles.backtestDisabled]}><Text style={styles.backtestRunText}>{busy ? "Replaying…" : "Run historical replay"}</Text></Pressable>
    {status ? <Text style={styles.disclaimer}>{status}</Text> : null}
    {result ? <>
      <View style={styles.backtestMetrics}>
        {[
          ["Net P/L", result.net_pnl.toFixed(4)], ["Return", result.net_return_pct.toFixed(3) + "%"],
          ["Max drawdown", result.max_drawdown_pct.toFixed(3) + "%"], ["Final equity", result.final_equity.toFixed(4)],
          ["Events", String(result.events_replayed)], ["Largest raw-event gap", (result.max_event_gap_ms / 1000).toFixed(1) + "s"],
          ["Gaps over 5 minutes", String(result.event_gaps_over_5m)], ["BUY / SELL / WAIT", result.buy_decisions + " / " + result.sell_decisions + " / " + result.wait_decisions],
          ["Fees", result.fees_paid.toFixed(4)], ["Slippage", result.slippage_paid.toFixed(4)],
          ["Net funding · received +", result.net_funding_cashflow.toFixed(4)], ["Funding settlements", String(result.funding_settlements)],
        ].map(([label, value]) => <View key={label} style={styles.backtestMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
      </View>
      <Text style={styles.disclaimer}>Strategy {result.strategy_key}@{result.strategy_version} · {result.venue} · {result.events_replayed} observations.</Text>
      {result.fills.map((fill, index) => <View key={fill.event_id + index} style={styles.backtestFill}>
        <Text style={styles.newsMeta}>{fill.action} · {new Date(fill.ts_ms).toLocaleString()}</Text>
        <Text style={styles.engineValue}>Price {fill.price.toPrecision(8)} · Qty {fill.quantity.toPrecision(6)}</Text>
        <Text style={styles.disclaimer}>Fee {fill.fee.toFixed(4)} · Slippage {fill.slippage_cost.toFixed(4)} · Realized {fill.realized_pnl.toFixed(4)}</Text>
      </View>)}
      <Text style={styles.traceJson}>{result.assumptions.join("\n")}</Text>
    </> : null}
  </View>;
}

export function ArchiveExploreScreen({ mode }: Props) {
  const [items, setItems] = useState(seed);
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [vector, setVector] = useState<QoinVectorDecision | null>(null);
  const [strategy, setStrategy] = useState<QoinStrategy | null>(null);
  const [catalogue, setCatalogue] = useState<QoinAlgorithmCatalogue | null>(null);
  const [catalogueStatus, setCatalogueStatus] = useState("loading");
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
  const [accountOpen, setAccountOpen] = useState(false);
  const [firebaseUser, setFirebaseUser] = useState<User | null>(null);
  const [avatarUrl, setAvatarUrl] = useState<string | null>(null);
  const [avatarBusy, setAvatarBusy] = useState(false);
  useEffect(() => { try { return subscribeQoinAuth((user) => { setFirebaseUser(user); setAvatarUrl(null); }); } catch { return undefined; } }, []);
  const chooseAvatar = async () => {
    const permission = await ImagePicker.requestMediaLibraryPermissionsAsync();
    if (!permission.granted) { Alert.alert("Photo access needed", "Allow Qoin to access your photo library to change your profile picture."); return; }
    const selection = await ImagePicker.launchImageLibraryAsync({ mediaTypes: ImagePicker.MediaTypeOptions.Images, allowsEditing: true, aspect: [1, 1], quality: 0.85 });
    if (selection.canceled || !selection.assets[0]) return;
    setAvatarBusy(true);
    try { const response = await fetch(selection.assets[0].uri); const blob = await response.blob(); const url = await uploadQoinAvatar(blob); setAvatarUrl(url); setAccountOpen(false); }
    catch (error) { Alert.alert("Avatar upload failed", error instanceof Error ? error.message : "Try another image."); }
    finally { setAvatarBusy(false); }
  };
  const previousActionRef = useRef<Record<string, string>>({});
  const notifiedEventIdsRef = useRef<Set<string>>(new Set());
  const unreadNotifications = notifications.filter((item) => !item.read).length;
  const addNotification = (notification: QoinNotification) => setNotifications((current) => current.some((item) => item.id === notification.id) ? current : [notification, ...current].slice(0, 100));
  const archived = useMemo(() => items.filter((item) => item.archived), [items]);
  const decisionStrategy = vector?.strategy_parameters || strategy;
  const discovered = useMemo(() => items.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [items, market, query]);
  const requireAuthentication = () => { setAccountOpen(true); Alert.alert("Sign in required", "Sign in to Qoin to change your archive or run a research replay."); };
  const toggle = (symbol: string) => {
    if (!firebaseUser) { requireAuthentication(); return; }
    setItems((current) => current.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
  };
  useEffect(() => { let active = true; getQoinAlgorithmCatalogue().then((data) => { if (active) { setCatalogue(data); setCatalogueStatus("loaded"); } }).catch(() => { if (active) setCatalogueStatus("unavailable"); }); return () => { active = false; }; }, []);
  useEffect(() => {
    let active = true;
    getQoinInstruments().then(({ items }) => {
      if (!active) return;
      const colors = ["#E6A23C", "#6477DF", "#8B72E8", "#4D91D8", "#76A85A", "#8895A7"];
      setItems(items.map((item, index) => ({
        symbol: item.symbol, engineSymbol: item.engine_symbol, name: item.name,
        market: item.asset_class, detail: item.description, venue: item.venue, engineVenue: item.venue_id || (item.asset_class === "Crypto" ? "binance_usdm" : "finnhub"), provider: item.provider, providerStatus: item.provider_status,
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
  useEffect(() => { getQoinStrategy().then(setStrategy).catch(() => setStrategy(null)); }, []);
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
    getInstrumentVectorHistory(selected.engineSymbol, 500).then((result) => { if (active) setVectorHistory(result.items); }).catch(() => undefined);
    getQoinEvents(selected.engineSymbol, 1000).then((result) => { if (active) setRawEvents(result.items.slice().reverse()); }).catch(() => undefined);
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
            setRawEvents((current) => [{
              ...update.event,
              event_id: update.event?.event_id || String(update.server_ts_ms || Date.now()),
              venue: update.event?.venue || "stream", symbol,
              event_type: update.event?.event_type || "unknown",
              ts_ms: update.event?.ts_ms || update.server_ts_ms || Date.now(),
            } as QoinMarketEvent, ...current].slice(0, 1000));
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
        <View style={styles.topline}><Text style={styles.brand}>qoin</Text><View style={styles.topActions}><Pressable accessibilityRole="button" accessibilityLabel="Open account menu" onPress={() => setAccountOpen(true)} style={styles.avatar}>{(avatarUrl || firebaseUser?.photoURL) ? <Image source={{ uri: avatarUrl || firebaseUser?.photoURL || "" }} style={styles.avatarImage} /> : <Text style={styles.avatarText}>{(firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</Text>}</Pressable></View></View>
        <View style={styles.hero}><Text style={styles.kicker}>YOUR MARKET WORKSPACE</Text><Text style={styles.heroTitle}>See the market{"\n"}<Text style={styles.blue}>as a system.</Text></Text><Text style={styles.heroCopy}>Keep the instruments that matter close. Explore the forces shaping each one.</Text></View>
        <View style={styles.headingRow}><View><Text style={styles.kicker}>{mode === "Archive" ? "YOUR COLLECTION" : "DISCOVERY"}</Text><Text style={styles.heading}>{mode}</Text><Text style={styles.subheading}>{mode === "Archive" ? "Every instrument you follow, in one place." : "Find instruments to bring into your archive."}</Text></View><Text style={styles.count}>{mode === "Archive" ? archived.length : items.length - archived.length} ASSETS</Text></View>

        {mode === "Explore" ? (
          <>
            <View style={styles.search}><Text style={styles.searchIcon}>⌕</Text><TextInput value={query} onChangeText={setQuery} placeholder="Search instruments or symbols" placeholderTextColor="#A4AFBD" style={styles.searchInput} /></View>
            <View style={styles.filters}>{["All", "Crypto", "Equities", "FX", "Indices", "Commodities"].map((name) => <Pressable key={name} onPress={() => setMarket(name)} style={[styles.filter, market === name && styles.filterActive]}><Text style={[styles.filterText, market === name && styles.filterTextActive]}>{name}</Text></Pressable>)}</View>
            <View style={styles.coverageSection}>
              <View style={styles.coverageHeading}><View style={styles.coverageHeadingCopy}><Text style={styles.kicker}>MARKET ACCESS DESIGN</Text><Text style={styles.newsTitle}>Coverage tiers</Text><Text style={styles.coverageSubheading}>Data breadth is separate from strategy performance and broker execution.</Text></View><Text style={styles.coverageStatus}>{catalogueStatus === "loaded" ? (catalogue?.market_access_tiers_enabled ? "ENABLED" : "PREVIEW") : catalogueStatus.toUpperCase()}</Text></View>
              {catalogue?.market_access_tiers?.length ? catalogue.market_access_tiers.map((tier) => <View key={tier.tier_id} style={styles.coverageCard}>
                <View style={styles.coverageCardTop}><Text style={styles.coverageName}>{tier.display_name}</Text><Text style={styles.coverageTag}>{tier.status}</Text></View>
                <Text style={styles.coveragePrimary}>{tier.max_active_instruments} <Text style={styles.coverageUnit}>active instruments</Text></Text>
                <Text style={styles.coverageLine}>Markets · {tier.asset_classes.join(" · ")}</Text>
                <Text style={styles.coverageLine}>History · {tier.historical_depth_days} days</Text>
                <Text style={styles.coverageLine}>Timeframes · {tier.supported_timeframes.join(" · ")}</Text>
                <Text style={styles.coverageLine}>Providers · {tier.market_data_providers.join(" · ") || "None listed"}</Text>
                <Text style={styles.coverageLine}>Execution brokers · {tier.execution_brokers.length ? tier.execution_brokers.join(" · ") : "None — research only"}</Text>
                <Text style={styles.coverageNote}>{tier.notes}</Text>
              </View>) : <Text style={styles.noteCopy}>{catalogueStatus === "loading" ? "Loading proposed market coverage…" : "Market coverage catalogue unavailable."}</Text>}
              <Text style={styles.coverageDisclaimer}>Preview only: not active subscriptions, prices or enforced permissions. Finnhub data depends on a configured key and eligible access. No live broker execution is connected.</Text>
            </View>
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
          {news.length ? news.map((item) => <Pressable key={String(item.id)} style={styles.newsItem} onPress={() => Linking.openURL(item.url).catch(() => undefined)}>
            <Text style={styles.newsMeta}>{item.source} · {item.published_at_ms ? new Date(item.published_at_ms).toLocaleString() : "Timestamp unavailable"}</Text>
            <Text style={styles.newsHeadline}>{item.headline}</Text><Text style={styles.newsSummary} numberOfLines={3}>{item.summary}</Text>
          </Pressable>) : <Text style={styles.noteCopy}>{newsStatus === "loading" ? "Loading sourced headlines…" : newsStatus + ". Configure FINNHUB_API_KEY on the backend. No placeholder headlines are generated."}</Text>}
        </View> : null}
        {mode === "Explore" ? <View style={styles.macroSection}>
          <Text style={styles.kicker}>UPCOMING CATALYSTS</Text><Text style={styles.newsTitle}>Economic calendar</Text>
          <Text style={styles.noteCopy}>Scheduled releases, with actual, estimate and previous values where published.</Text>
          {macroEvents.length ? macroEvents.map((item,index) => <View key={item.time + item.country + item.event + index} style={styles.macroItem}>
            <Text style={styles.newsHeadline}>{item.event}</Text><Text style={styles.newsMeta}>{item.country} · {item.time} · {item.impact || "Impact not supplied"}</Text>
            <View style={styles.macroValues}><Text style={styles.newsMeta}>Actual: {item.actual ?? "—"}</Text><Text style={styles.newsMeta}>Estimate: {item.estimate ?? "—"}</Text><Text style={styles.newsMeta}>Previous: {item.previous ?? "—"}</Text></View>
          </View>) : <Text style={styles.noteCopy}>{calendarStatus === "loading" ? "Loading economic calendar…" : calendarStatus + ". Configure FINNHUB_API_KEY to load macro events. No event dates are fabricated."}</Text>}
        </View> : null}

        <Text style={styles.footer}>QOIN · RESEARCH MODE · NO LIVE EXECUTION</Text>
      </ScrollView>

      <Modal visible={accountOpen} transparent animationType="fade" onRequestClose={() => setAccountOpen(false)}>
        <Pressable style={styles.accountShade} onPress={() => setAccountOpen(false)}><Pressable style={styles.accountSheet} onPress={(event) => event.stopPropagation()}>
          <View style={styles.accountIdentity}><View style={styles.accountAvatar}>{(avatarUrl || firebaseUser?.photoURL) ? <Image source={{ uri: avatarUrl || firebaseUser?.photoURL || "" }} style={styles.accountAvatarImage} /> : <Text style={styles.avatarText}>{(firebaseUser?.displayName || firebaseUser?.email || "Q").slice(0,1).toUpperCase()}</Text>}</View><View style={{ flex: 1 }}><Text style={styles.accountName}>{firebaseUser?.displayName || "Qoin member"}</Text><Text style={styles.accountEmail}>{firebaseUser?.email || "Signed in"}</Text></View></View>
          <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); Alert.alert("Profile", firebaseUser?.email || "Your Qoin profile"); }}><Text style={styles.accountActionText}>Profile</Text></Pressable>
          <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); setNotificationsOpen(true); }}><Text style={styles.accountActionText}>Notifications {unreadNotifications ? "· " + unreadNotifications + " unread" : ""}</Text></Pressable>
          <Pressable style={styles.accountAction} disabled={avatarBusy} onPress={() => void chooseAvatar()}><Text style={styles.accountActionText}>{avatarBusy ? "Uploading picture…" : "Change profile picture"}</Text></Pressable>
          <Pressable style={styles.accountAction} onPress={() => { setAccountOpen(false); void signOutQoinUser().catch((error) => Alert.alert("Sign out failed", error instanceof Error ? error.message : "Try again.")); }}><Text style={styles.accountActionText}>Sign out</Text></Pressable>
        </Pressable></Pressable>
      </Modal>
      <Modal visible={notificationsOpen} transparent animationType="slide" onRequestClose={() => setNotificationsOpen(false)}>
        <View style={styles.modalShade}><View style={styles.notificationModal}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>LIVE ENGINE EVENTS</Text><Text style={styles.modalTitle}>Notifications</Text><Text style={styles.assetMeta}>{unreadNotifications} unread · current session</Text></View><Pressable onPress={() => setNotificationsOpen(false)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View>
          <View style={styles.notificationActions}><Pressable onPress={() => setNotifications((current) => current.map((item) => ({ ...item, read: true })))}><Text style={styles.notificationActionText}>Mark all read</Text></Pressable><Pressable onPress={() => setNotifications([])}><Text style={styles.notificationActionText}>Clear history</Text></Pressable></View>
          <ScrollView contentContainerStyle={styles.notificationContent}>{notifications.length ? notifications.map((item) => <View key={item.id} style={[styles.notificationItem, item.read && styles.notificationRead]}><Text style={styles.notificationMeta}>{item.kind.toUpperCase()} · {item.symbol} · {new Date(item.createdAt).toLocaleTimeString()}</Text><Text style={styles.notificationTitle}>{item.title}</Text><Text style={styles.notificationBody}>{item.body}</Text><Text style={styles.notificationPayload}>{item.details}</Text></View>) : <Text style={styles.noteCopy}>No alerts yet. Signal transitions and liquidation events from the live stream will appear here.</Text>}</ScrollView>
        </View></View>
      </Modal>
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
                  ["Signed flow", marketState?.signed_flow?.toFixed(4) ?? "—"],
                  ["Order flow", marketState?.order_flow_imbalance?.toFixed(4) ?? "—"],
                  ["Funding", marketState?.funding_rate?.toFixed(6) ?? "—"],
                  ["Open interest", marketState?.open_interest?.toString() ?? "—"],
                  ["Liquidations", marketState?.liquidation_events?.toString() ?? "—"],
                  ["Last liquidation side", marketState?.last_liquidation_side ?? "—"],
                  ["Events", marketState?.event_count?.toString() ?? "—"],
                  ["Total trade size", marketState?.total_trade_size?.toFixed(4) ?? "—"],
                  ["Event intensity", marketState?.event_intensity?.toFixed(3) ?? "—"],
                  ["EWMA abs return", marketState?.ewma_abs_return?.toFixed(6) ?? "—"],
                  ["Last event time", marketState?.last_event_ts_ms ? new Date(marketState.last_event_ts_ms).toLocaleTimeString() : "—"],
                  ["State quality", marketState?.state_quality?.toFixed(3) ?? "—"],
                ].map(([label,value]) => <View key={label} style={styles.liveMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
              </View>
              <Text style={styles.disclaimer}>Last raw event: {lastEvent ? JSON.stringify(lastEvent) : "none received"}. Missing values mean the provider has not supplied that field.</Text>
            </View>
            <FeatureSpace decision={vector} />
            {vector ? <DecisionTracePanel decision={vector} /> : null}
            <MobileBacktestPanel symbol={selected.engineSymbol} venue={selected.engineVenue} authenticated={Boolean(firebaseUser)} onRequireAuth={requireAuthentication} />
            <View style={styles.strategyPanel}>
              <View style={styles.liveHeading}><Text style={styles.attentionTitle}>Active strategy map</Text><Text style={styles.attentionCaption}>{decisionStrategy?.version || "Unavailable"}</Text></View>
              {decisionStrategy ? <>
                <View style={styles.strategyGates}>
                  {[
                    ["Angle", decisionStrategy.angular_threshold_degrees.toFixed(2) + "°"],
                    ["Magnitude", decisionStrategy.magnitude_threshold.toFixed(3)],
                    ["Max spread", decisionStrategy.max_spread_bps.toFixed(2) + " bps"],
                    ["Attention", decisionStrategy.attention_scale.toFixed(3)],
                  ].map(([label,value]) => <View key={label} style={styles.liveMetric}><Text style={styles.engineLabel}>{label}</Text><Text style={styles.engineValue}>{value}</Text></View>)}
                </View>
                {QOIN_FEATURE_NAMES.map((name) => <View key={name} style={styles.strategyWeightRow}>
                  <Text style={styles.metricLabel}>{name.replaceAll("_"," ")}</Text>
                  <View style={styles.strategyWeightValues}>
                    <Text style={styles.attentionCaption}>Bullish {decisionStrategy.bullish_weights[name]?.toFixed(3) ?? "—"}</Text>
                    <Text style={styles.attentionCaption}>Bearish {decisionStrategy.bearish_weights[name]?.toFixed(3) ?? "—"}</Text>
                    <Text style={styles.attentionCaption}>Scale {decisionStrategy.feature_scales[name]?.toFixed(3) ?? "—"}</Text>
                  </View>
                </View>)}
                <Text style={styles.disclaimer}>Status: {decisionStrategy.status}. Last update: {new Date(decisionStrategy.updated_at_ms).toLocaleString()}.</Text>
              </> : <Text style={styles.disclaimer}>The active strategy map could not be loaded.</Text>}
            </View>
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
                <View style={styles.contributionPanel}>
                  <View style={styles.attentionHeader}><Text style={styles.attentionTitle}>Feature contribution ledger</Text><Text style={styles.attentionCaption}>Weight × attended feature</Text></View>
                  {QOIN_FEATURE_NAMES.map((name,index) => {
                    const attended = vector.attended_features?.[index] ?? 0;
                    const bullishWeight = strategy?.bullish_weights[name];
                    const bearishWeight = strategy?.bearish_weights[name];
                    return <View key={name} style={styles.contributionRow}>
                      <Text style={styles.metricLabel}>{name.replaceAll("_"," ")}</Text>
                      <View style={styles.strategyWeightValues}>
                        <Text style={styles.attentionCaption}>{bullishWeight == null ? "Bullish weight unavailable" : "Bull " + bullishWeight.toFixed(3) + " × " + attended.toFixed(3) + " = " + (bullishWeight*attended).toFixed(4)}</Text>
                        <Text style={styles.attentionCaption}>{bearishWeight == null ? "Bearish weight unavailable" : "Bear " + bearishWeight.toFixed(3) + " × " + attended.toFixed(3) + " = " + (bearishWeight*attended).toFixed(4)}</Text>
                      </View>
                    </View>;
                  })}
                  <Text style={styles.disclaimer}>Signed contributions are shown before the aggregate positive clamp. The actual decision vector remains authoritative.</Text>
                </View>
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
              {showRawEvents ? rawEvents.slice(0,1000).map((event) => <View key={event.event_id} style={styles.rawEvent}>
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
  tracePanel: { gap: 10, padding: 13, borderRadius: 16, borderWidth: 1, borderColor: "#E4EAF3", backgroundColor: "#FFFFFF" },
  mobileBacktest: { gap: 11, padding: 13, borderRadius: 15, borderWidth: 1, borderColor: "#E4EAF3", backgroundColor: "#FFFFFF" },
  backtestInputs: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  backtestInputWrap: { flexGrow: 1, flexBasis: "30%", minWidth: 85, gap: 5 },
  backtestInput: { borderWidth: 1, borderColor: "#E1E8F1", borderRadius: 9, paddingHorizontal: 9, paddingVertical: 7, color: "#24344D", fontSize: 12 },
  backtestRun: { alignItems: "center", paddingVertical: 11, borderRadius: 22, backgroundColor: "#2867DC" },
  backtestDisabled: { opacity: 0.55 },
  backtestRunText: { color: "#FFFFFF", fontWeight: "800", fontSize: 11 },
  backtestMetrics: { flexDirection: "row", flexWrap: "wrap", gap: 7 },
  backtestMetric: { width: "48%", gap: 4, padding: 9, borderRadius: 9, backgroundColor: "#F5F8FD" },
  backtestFill: { paddingVertical: 8, borderBottomWidth: 1, borderBottomColor: "#E8EDF5", gap: 4 },
  traceSource: { gap: 4, padding: 10, borderRadius: 10, backgroundColor: "#F5F8FD" },
  traceValue: { color: "#30445F", fontSize: 10, fontWeight: "700" },
  traceTableRow: { flexDirection: "row", borderBottomWidth: 1, borderBottomColor: "#E8EDF5" },
  traceCell: { width: 104, paddingHorizontal: 7, paddingVertical: 8, color: "#52647C", fontSize: 8 },
  traceHead: { color: "#30445F", fontWeight: "800", backgroundColor: "#F5F8FD" },
  traceGateGrid: { flexDirection: "row", flexWrap: "wrap", gap: 7 },
  traceGate: { width: "48%", gap: 4, padding: 9, borderRadius: 9, backgroundColor: "#F5F8FD" },
  traceSubhead: { color: "#30445F", fontSize: 10, fontWeight: "800", marginTop: 5 },
  traceJson: { color: "#52647C", backgroundColor: "#F5F7FB", padding: 9, borderRadius: 9, fontSize: 8, lineHeight: 12 },

  screen: { flex: 1, backgroundColor: "#F7F9FC" },
  topActions: { flexDirection: "row", alignItems: "center", gap: 10 },
  notificationButton: { width: 32, height: 32, borderRadius: 16, backgroundColor: "#EDF3FF", alignItems: "center", justifyContent: "center" },
  notificationGlyph: { color: "#2867DC", fontSize: 18 },
  notificationBadge: { position: "absolute", right: -5, top: -5, minWidth: 15, height: 15, paddingHorizontal: 3, borderRadius: 8, backgroundColor: "#2867DC", alignItems: "center", justifyContent: "center" },
  notificationBadgeText: { color: "#FFFFFF", fontSize: 7, fontWeight: "800" },
  notificationModal: { maxHeight: "88%", backgroundColor: "#FFFFFF", borderTopLeftRadius: 21, borderTopRightRadius: 21, overflow: "hidden" },
  notificationActions: { flexDirection: "row", justifyContent: "flex-end", gap: 18, paddingHorizontal: 18, paddingVertical: 10, borderBottomWidth: 1, borderBottomColor: "#EDF1F6" },
  notificationActionText: { color: "#2867DC", fontSize: 10, fontWeight: "700" },
  notificationContent: { padding: 14 },
  notificationItem: { paddingVertical: 12, borderBottomWidth: 1, borderBottomColor: "#EDF1F6" },
  notificationRead: { opacity: 0.6 },
  notificationMeta: { color: "#8996A8", fontSize: 8, fontWeight: "800" },
  notificationTitle: { color: "#30445F", fontSize: 11, fontWeight: "800", marginTop: 6 },
  notificationBody: { color: "#8391A4", fontSize: 9, lineHeight: 14, marginTop: 4 },
  notificationPayload: { color: "#8A97A9", fontSize: 7, lineHeight: 11, marginTop: 7 },

  content: { paddingHorizontal: 17, paddingBottom: 25 },
  topline: { height: 42, flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  brand: { color: "#1B2C45", fontSize: 21, fontWeight: "900", letterSpacing: -1 },
  avatar: { width: 29, height: 29, borderRadius: 15, backgroundColor: "#E8EFFF", alignItems: "center", justifyContent: "center" },
  avatarText: { color: "#2867DC", fontWeight: "800" },
  avatarImage: { width: 29, height: 29, borderRadius: 15 },
  accountShade: { flex: 1, justifyContent: "flex-start", alignItems: "flex-end", paddingTop: 58, paddingHorizontal: 16, backgroundColor: "#17294344" },
  accountSheet: { width: 270, padding: 12, borderRadius: 19, backgroundColor: "#FFFFFF" },
  accountIdentity: { flexDirection: "row", alignItems: "center", gap: 10, padding: 9, marginBottom: 4 },
  accountAvatar: { width: 38, height: 38, borderRadius: 19, backgroundColor: "#E8EFFF", alignItems: "center", justifyContent: "center", overflow: "hidden" },
  accountAvatarImage: { width: 38, height: 38, borderRadius: 19 },
  accountName: { color: "#24344D", fontSize: 12, fontWeight: "800" },
  accountEmail: { color: "#8794A7", fontSize: 9, marginTop: 3 },
  accountAction: { paddingVertical: 12, paddingHorizontal: 10, borderRadius: 11 },
  accountActionText: { color: "#43546C", fontSize: 11, fontWeight: "600" },
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
  coverageSection: { marginTop: 8, marginBottom: 18, paddingTop: 15, borderTopWidth: 1, borderTopColor: "#E8EDF4", borderBottomWidth: 1, borderBottomColor: "#E8EDF4", paddingBottom: 14 },
  coverageHeading: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 8, marginBottom: 11 },
  coverageHeadingCopy: { flex: 1 },
  coverageSubheading: { color: "#8794A7", fontSize: 9, lineHeight: 14, marginTop: 3 },
  coverageStatus: { color: "#2867D8", backgroundColor: "#EDF3FF", borderRadius: 14, overflow: "hidden", paddingHorizontal: 8, paddingVertical: 5, fontSize: 8, fontWeight: "800" },
  coverageCard: { padding: 12, backgroundColor: "#FFFFFF", borderWidth: 1, borderColor: "#E7EDF5", borderRadius: 12, marginBottom: 8 },
  coverageCardTop: { flexDirection: "row", alignItems: "center", justifyContent: "space-between" },
  coverageName: { color: "#243750", fontSize: 12, fontWeight: "800" },
  coverageTag: { color: "#8A97A9", fontSize: 8, textTransform: "uppercase" },
  coveragePrimary: { color: "#2867D8", fontSize: 23, fontWeight: "800", marginVertical: 8 },
  coverageUnit: { color: "#8794A7", fontSize: 9, fontWeight: "500" },
  coverageLine: { color: "#52637A", fontSize: 9, lineHeight: 14, marginTop: 4 },
  coverageNote: { color: "#8B98AA", fontSize: 9, lineHeight: 14, marginTop: 8 },
  coverageDisclaimer: { color: "#8B98AA", fontSize: 9, lineHeight: 14, marginTop: 5 },
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
  strategyPanel: { marginTop: 14, padding: 10, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, backgroundColor: "#FBFCFE" },
  contributionPanel: { marginTop: 14, padding: 10, borderWidth: 1, borderColor: "#E9EEF5", borderRadius: 11, backgroundColor: "#FBFCFE" },
  contributionRow: { paddingVertical: 8, borderTopWidth: 1, borderTopColor: "#EDF1F6" },
  strategyGates: { flexDirection: "row", flexWrap: "wrap", gap: 10, marginVertical: 10 },
  strategyWeightRow: { paddingVertical: 8, borderTopWidth: 1, borderTopColor: "#EDF1F6" },
  strategyWeightValues: { flexDirection: "row", justifyContent: "space-between", flexWrap: "wrap", gap: 6, marginTop: 5 },
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
