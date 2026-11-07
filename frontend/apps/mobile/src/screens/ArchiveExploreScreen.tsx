import React, { useEffect, useMemo, useState } from "react";
import { Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { getOperatingCycle, getPaperSession, getPaperSessionHistory, getPaperSessionRuns, type OperatingCycleSnapshot, type PaperSessionSnapshot, type PaperMirrorEvent, type PaperTradeHistoryItem } from "@qoin/core";
import { colors } from "../theme";

type Tab = "Archive" | "Explore";
type Instrument = { symbol: string; venue: string; currency: string; status: string; pnl?: number };
const directory = [
  { symbol: "EURUSD", name: "Euro / US Dollar", category: "Forex" },
  { symbol: "GBPUSD", name: "British Pound / US Dollar", category: "Forex" },
  { symbol: "USDJPY", name: "US Dollar / Japanese Yen", category: "Forex" },
  { symbol: "BTCUSDT", name: "Bitcoin", category: "Crypto" },
  { symbol: "ETHUSDT", name: "Ethereum", category: "Crypto" },
  { symbol: "XAUUSD", name: "Gold", category: "Commodities" },
  { symbol: "XAGUSD", name: "Silver", category: "Commodities" },
  { symbol: "NVDA", name: "NVIDIA", category: "Stocks" },
  { symbol: "AAPL", name: "Apple", category: "Stocks" },
];

export function ArchiveExploreScreen() {
  const [tab, setTab] = useState<Tab>("Archive");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const [cycle, setCycle] = useState<OperatingCycleSnapshot | null>(null);
  const [session, setSession] = useState<PaperSessionSnapshot | null>(null);
  const [decisions, setDecisions] = useState<PaperMirrorEvent[]>([]);
  const [trades, setTrades] = useState<PaperTradeHistoryItem[]>([]);
  const [saved, setSaved] = useState<string[]>([]);
  const [category, setCategory] = useState("All");
  const [query, setQuery] = useState("");

  useEffect(() => {
    const refresh = () => {
      getOperatingCycle().then(setCycle).catch(() => setCycle(null));
      getPaperSession().then(setSession).catch(() => setSession(null));
      getPaperSessionHistory(undefined, 100).then((result) => { setDecisions(result.decisions); setTrades(result.trades); }).catch(() => { setDecisions([]); setTrades([]); });
      getPaperSessionRuns(20).catch(() => undefined);
    };
    refresh();
    const timer = setInterval(refresh, 5000);
    return () => clearInterval(timer);
  }, []);

  const instruments = useMemo<Instrument[]>(() => {
    const map = new Map<string, Instrument>();
    for (const account of cycle?.config.paper_accounts ?? []) map.set(account.venue + ":" + account.symbol, { symbol: account.symbol, venue: account.venue, currency: account.quote_currency, status: "Configured" });
    for (const account of session?.accounts ?? []) map.set(account.venue + ":" + account.symbol, { symbol: account.symbol, venue: account.venue, currency: account.quote_currency, status: "Paper account", pnl: account.net_pnl });
    for (const symbol of saved) if (![...map.values()].some((item) => item.symbol === symbol)) map.set("discovery:" + symbol, { symbol, venue: "Not configured", currency: symbol.endsWith("USDT") ? "USDT" : "USD", status: "Added · discovery" });
    return [...map.values()].sort((a, b) => a.symbol.localeCompare(b.symbol));
  }, [cycle, session, saved]);

  const events = selected ? decisions.filter((item) => item.symbol === selected.symbol && item.venue === selected.venue) : [];
  const latest = events[0];
  const selectedTrades = selected ? trades.filter((item) => item.symbol === selected.symbol && item.venue === selected.venue) : [];
  const found = directory.filter((item) => (category === "All" || item.category === category) && (item.symbol + item.name).toLowerCase().includes(query.toLowerCase()));

  if (selected) return (
    <SafeAreaView style={styles.screen}>
      <ScrollView contentContainerStyle={styles.content}>
        <Pressable onPress={() => setSelected(null)}><Text style={styles.back}>‹  Back to {tab}</Text></Pressable>
        <Text style={styles.kicker}>INSTRUMENT SPACE / {selected.venue}</Text>
        <Text style={styles.title}>{selected.symbol}</Text>
        <Text style={styles.subtitle}>{selected.currency} quote currency · {selected.status}</Text>
        <View style={styles.panel}>
          <Text style={styles.panelTitle}>Vector space</Text>
          <Text style={styles.helper}>Feature-space projection from saved Mirror observations</Text>
          <View style={styles.vector}>
            <View style={styles.vectorRing} /><View style={[styles.vectorRing, styles.ringMiddle]} /><View style={[styles.vectorRing, styles.ringInner]} />
            {(Object.entries(latest?.features ?? {}).slice(0, 7)).map(([name, value], index) => {
              const angle = (Math.PI * 2 * index) / Math.max(Object.keys(latest?.features ?? {}).length, 1) - Math.PI / 2;
              const radius = 34 + Math.min(1, Math.abs(value)) * 54;
              return <View key={name} style={[styles.vectorNode, { left: 50 + Math.cos(angle) * radius + 90, top: 85 + Math.sin(angle) * radius }]}><View style={styles.nodeDot} /><Text style={styles.nodeLabel}>{name.replace(/_/g, " ")}</Text></View>;
            })}
            <View style={styles.vectorCenter} />
          </View>
          {latest ? <><View style={styles.metricRow}><Text style={styles.metricLabel}>Decision / execution</Text><Text style={styles.metricValue}>{latest.action.toUpperCase()} · {latest.execution}</Text></View><View style={styles.metricRow}><Text style={styles.metricLabel}>Magnitude / angle</Text><Text style={styles.metricValue}>{latest.magnitude.toFixed(3)} / {latest.angle_radians.toFixed(3)}</Text></View>{Object.entries(latest.features).map(([key, value]) => <View style={styles.metricRow} key={key}><Text style={styles.metricLabel}>{key.replace(/_/g, " ")}</Text><Text style={styles.metricValue}>{value.toFixed(3)}</Text></View>)}</> : <Text style={styles.helper}>No Mirror observations yet. Configure this exact venue/symbol feed to populate its vector state.</Text>}
        </View>
        <View style={styles.panel}><Text style={styles.panelTitle}>Mirror history</Text>{events.slice(0, 12).map((item) => <View style={styles.historyRow} key={item.event_id}><Text style={styles.metricValue}>{item.action.toUpperCase()} · {item.magnitude.toFixed(3)}</Text><Text style={styles.helper}>{item.reason}</Text></View>)}{selectedTrades.slice(0, 8).map((item, index) => <View style={styles.historyRow} key={item.entry_event_id + index}><Text style={styles.metricValue}>TRADE · {item.net_pnl.toFixed(2)} {item.quote_currency}</Text><Text style={styles.helper}>Fees {item.fees.toFixed(2)} · Return {item.return_pct.toFixed(2)}%</Text></View>)}{!events.length && !selectedTrades.length ? <Text style={styles.helper}>No instrument activity recorded.</Text> : null}</View>
      </ScrollView>
    </SafeAreaView>
  );

  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.header}><View><Text style={styles.kicker}>QOIN / MARKET INTELLIGENCE</Text><Text style={styles.title}>{tab}</Text></View><View style={styles.status}><View style={styles.statusDot} /><Text style={styles.helper}>Paper research</Text></View></View>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        {tab === "Archive" ? <>
          <Text style={styles.subtitle}>Your instruments, each with its own evolving vector space.</Text>
          {instruments.map((item) => {
            const event = decisions.find((entry) => entry.symbol === item.symbol && entry.venue === item.venue);
            return <Pressable key={item.venue + item.symbol} style={styles.instrumentCard} onPress={() => setSelected(item)}>
              <View style={styles.assetIcon}><Text style={styles.assetLetter}>{item.symbol.slice(0, 1)}</Text></View>
              <View style={styles.assetInfo}><Text style={styles.assetTitle}>{item.symbol}</Text><Text style={styles.helper}>{item.venue} · {item.currency}</Text><Text style={styles.helper}>{event ? "Vector updated " + new Date(event.ts_ms).toLocaleTimeString() : item.status}</Text></View>
              <View style={styles.assetRight}><Text style={styles.assetValue}>{item.pnl == null ? "—" : item.pnl.toFixed(2)}</Text><Text style={styles.kicker}>{event?.action?.toUpperCase() ?? "OPEN"}</Text></View>
            </Pressable>;
          })}
          {!instruments.length ? <View style={styles.empty}><Text style={styles.panelTitle}>Your Archive starts here</Text><Text style={styles.helper}>Explore instruments and add the ones you want Qoin to observe.</Text><Pressable style={styles.primary} onPress={() => setTab("Explore")}><Text style={styles.primaryText}>Explore instruments</Text></Pressable></View> : null}
        </> : <>
          <Text style={styles.subtitle}>Discover instruments and market context.</Text>
          <View style={styles.newsCard}><Text style={styles.kicker}>MARKET BRIEFING</Text><Text style={styles.panelTitle}>News and market context</Text><Text style={styles.helper}>A news provider is not connected yet. Sourced headlines and market-moving events will appear here when integrated.</Text></View>
          <Text style={styles.panelTitle}>Instrument directory</Text>
          <View style={styles.filters}>{["All", "Forex", "Crypto", "Commodities", "Stocks"].map((item) => <Pressable key={item} onPress={() => setCategory(item)} style={[styles.filter, category === item && styles.filterActive]}><Text style={[styles.filterText, category === item && styles.filterTextActive]}>{item}</Text></Pressable>)}</View>
          <TextInput value={query} onChangeText={setQuery} placeholder="Search instruments…" placeholderTextColor={colors.textMuted} style={styles.search} />
          {found.map((item) => <View key={item.symbol} style={styles.instrumentCard}><View style={styles.assetIcon}><Text style={styles.assetLetter}>{item.symbol.slice(0, 1)}</Text></View><View style={styles.assetInfo}><Text style={styles.assetTitle}>{item.symbol}</Text><Text style={styles.helper}>{item.name} · {item.category}</Text></View><Pressable disabled={saved.includes(item.symbol) || instruments.some((savedItem) => savedItem.symbol === item.symbol)} onPress={() => setSaved((current) => [...current, item.symbol])} style={styles.addButton}><Text style={styles.addText}>{saved.includes(item.symbol) || instruments.some((savedItem) => savedItem.symbol === item.symbol) ? "Added" : "+ Add"}</Text></Pressable></View>)}
        </>}
      </ScrollView>
      <View style={styles.floatingTabs}>{(["Archive", "Explore"] as const).map((item) => <Pressable key={item} onPress={() => { setSelected(null); setTab(item); }} style={[styles.tab, tab === item && styles.tabActive]}><Text style={[styles.tabText, tab === item && styles.tabTextActive]}>{item === "Archive" ? "◈" : "⌕"}  {item}</Text></Pressable>)}</View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary },
  content: { paddingHorizontal: 16, paddingTop: 8, paddingBottom: 110 },
  header: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", paddingHorizontal: 16, paddingTop: 8, paddingBottom: 12, backgroundColor: colors.bgSecondary },
  kicker: { color: colors.accentCyan, fontSize: 9, fontWeight: "900", letterSpacing: 1.2, marginBottom: 5 },
  title: { color: colors.textPrimary, fontSize: 27, fontWeight: "900", letterSpacing: -0.8 },
  subtitle: { color: colors.textMuted, fontSize: 12, lineHeight: 18, marginBottom: 16 },
  helper: { color: colors.textMuted, fontSize: 10, lineHeight: 15 },
  status: { flexDirection: "row", alignItems: "center", gap: 7 },
  statusDot: { width: 7, height: 7, borderRadius: 4, backgroundColor: colors.accentCyan },
  panel: { backgroundColor: colors.bgSecondary, borderRadius: 16, padding: 16, marginBottom: 14, borderWidth: 1, borderColor: colors.border },
  panelTitle: { color: colors.textPrimary, fontSize: 16, fontWeight: "800", marginBottom: 8 },
  instrumentCard: { flexDirection: "row", alignItems: "center", gap: 11, padding: 13, borderRadius: 15, backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, marginBottom: 9 },
  assetIcon: { width: 40, height: 40, borderRadius: 20, backgroundColor: "#EAF1FF", alignItems: "center", justifyContent: "center" },
  assetLetter: { color: colors.accentCyan, fontWeight: "900", fontSize: 15 },
  assetInfo: { flex: 1, gap: 3 },
  assetTitle: { color: colors.textPrimary, fontSize: 13, fontWeight: "900" },
  assetRight: { alignItems: "flex-end", gap: 5 },
  assetValue: { color: colors.textPrimary, fontSize: 11, fontWeight: "800" },
  empty: { backgroundColor: colors.bgSecondary, padding: 20, borderRadius: 16, alignItems: "center", gap: 10 },
  primary: { backgroundColor: colors.accentCyan, paddingHorizontal: 17, paddingVertical: 11, borderRadius: 22 },
  primaryText: { color: colors.white, fontSize: 11, fontWeight: "800" },
  newsCard: { backgroundColor: colors.bgSecondary, borderRadius: 16, borderWidth: 1, borderColor: colors.border, padding: 17, marginBottom: 22 },
  filters: { flexDirection: "row", flexWrap: "wrap", gap: 7, marginBottom: 12 },
  filter: { paddingHorizontal: 11, paddingVertical: 8, borderRadius: 18, backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border },
  filterActive: { backgroundColor: "#EAF1FF", borderColor: "#D7E5FF" },
  filterText: { color: colors.textMuted, fontSize: 10, fontWeight: "700" },
  filterTextActive: { color: colors.accentCyan },
  search: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 11, paddingHorizontal: 13, paddingVertical: 10, color: colors.textPrimary, marginBottom: 12, fontSize: 12 },
  addButton: { paddingHorizontal: 13, paddingVertical: 9, backgroundColor: "#EAF1FF", borderRadius: 18 },
  addText: { color: colors.accentCyan, fontSize: 10, fontWeight: "800" },
  floatingTabs: { position: "absolute", bottom: 16, alignSelf: "center", flexDirection: "row", gap: 4, backgroundColor: colors.bgSecondary, padding: 5, borderRadius: 30, borderWidth: 1, borderColor: colors.border, elevation: 8 },
  tab: { paddingHorizontal: 17, paddingVertical: 11, borderRadius: 24 },
  tabActive: { backgroundColor: "#EAF1FF" },
  tabText: { color: colors.textMuted, fontSize: 11, fontWeight: "800" },
  tabTextActive: { color: colors.accentCyan },
  back: { color: colors.accentCyan, fontWeight: "800", fontSize: 12, marginBottom: 18 },
  vector: { height: 210, backgroundColor: "#F7FAFF", borderRadius: 14, marginVertical: 12, overflow: "hidden", alignItems: "center", justifyContent: "center" },
  vectorRing: { position: "absolute", width: 190, height: 190, borderWidth: 1, borderColor: "#DFE9F7", borderRadius: 100 },
  ringMiddle: { width: 130, height: 130 },
  ringInner: { width: 70, height: 70 },
  vectorNode: { position: "absolute", alignItems: "center", maxWidth: 75 },
  nodeDot: { width: 9, height: 9, borderRadius: 5, backgroundColor: colors.accentCyan, borderWidth: 2, borderColor: colors.white },
  nodeLabel: { color: "#647994", fontSize: 7, fontWeight: "700", textAlign: "center" },
  vectorCenter: { width: 12, height: 12, borderRadius: 7, backgroundColor: "#174EA6", borderWidth: 2, borderColor: colors.white },
  metricRow: { flexDirection: "row", justifyContent: "space-between", gap: 12, paddingVertical: 10, borderBottomWidth: 1, borderBottomColor: colors.border },
  metricLabel: { color: colors.textMuted, fontSize: 10, textTransform: "capitalize" },
  metricValue: { color: colors.textPrimary, fontSize: 10, fontWeight: "800", maxWidth: "62%", textAlign: "right" },
  historyRow: { paddingVertical: 10, borderBottomWidth: 1, borderBottomColor: colors.border, gap: 4 },
});
