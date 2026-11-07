import React, { useMemo, useState } from "react";
import { Modal, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import Svg, { Circle, Line, Rect } from "react-native-svg";
import { SafeAreaView } from "react-native-safe-area-context";

type Market = "Crypto" | "Equities" | "FX";
type Instrument = {
  symbol: string;
  name: string;
  market: Market;
  detail: string;
  archived: boolean;
  tint: string;
  vector: { flow: number; liquidity: number; volatility: number; positioning: number };
};

const seed: Instrument[] = [
  { symbol: "BTC/USD", name: "Bitcoin", market: "Crypto", detail: "Digital asset · USD", archived: true, tint: "#E6A23C", vector: { flow: .72, liquidity: .48, volatility: .64, positioning: -.22 } },
  { symbol: "ETH/USD", name: "Ethereum", market: "Crypto", detail: "Smart-contract network · USD", archived: true, tint: "#6477DF", vector: { flow: .36, liquidity: .62, volatility: .42, positioning: .18 } },
  { symbol: "SOL/USD", name: "Solana", market: "Crypto", detail: "Layer-one network · USD", archived: true, tint: "#8B72E8", vector: { flow: -.18, liquidity: .24, volatility: .78, positioning: .52 } },
  { symbol: "NVDA", name: "NVIDIA", market: "Equities", detail: "Semiconductors · NASDAQ", archived: false, tint: "#76A85A", vector: { flow: .58, liquidity: .82, volatility: .46, positioning: .31 } },
  { symbol: "EUR/USD", name: "Euro / US Dollar", market: "FX", detail: "Major currency pair", archived: false, tint: "#4D91D8", vector: { flow: .12, liquidity: .91, volatility: .19, positioning: -.08 } },
  { symbol: "AAPL", name: "Apple", market: "Equities", detail: "Consumer technology · NASDAQ", archived: false, tint: "#8895A7", vector: { flow: .28, liquidity: .86, volatility: .27, positioning: .14 } },
];

type Props = { mode: "Archive" | "Explore" };

export function ArchiveExploreScreen({ mode }: Props) {
  const [items, setItems] = useState(seed);
  const [query, setQuery] = useState("");
  const [market, setMarket] = useState("All");
  const [selected, setSelected] = useState<Instrument | null>(null);
  const archived = useMemo(() => items.filter((item) => item.archived), [items]);
  const discovered = useMemo(() => items.filter((item) => !item.archived && (market === "All" || item.market === market) && `${item.name} ${item.symbol}`.toLowerCase().includes(query.toLowerCase())), [items, market, query]);
  const toggle = (symbol: string) => {
    setItems((current) => current.map((item) => item.symbol === symbol ? { ...item, archived: !item.archived } : item));
    setSelected((current) => current?.symbol === symbol ? { ...current, archived: !current.archived } : current);
  };
  const data = mode === "Archive" ? archived : discovered;

  return (
    <SafeAreaView style={styles.screen} edges={["top"]}>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        <View style={styles.topline}><Text style={styles.brand}>qoin</Text><View style={styles.avatar}><Text style={styles.avatarText}>Q</Text></View></View>
        <View style={styles.hero}><Text style={styles.kicker}>YOUR MARKET WORKSPACE</Text><Text style={styles.heroTitle}>See the market{"\n"}<Text style={styles.blue}>as a system.</Text></Text><Text style={styles.heroCopy}>Keep the instruments that matter close. Explore the forces shaping each one.</Text></View>
        <View style={styles.headingRow}><View><Text style={styles.kicker}>{mode === "Archive" ? "YOUR COLLECTION" : "DISCOVERY"}</Text><Text style={styles.heading}>{mode}</Text><Text style={styles.subheading}>{mode === "Archive" ? "Every instrument you follow, in one place." : "Find instruments to bring into your archive."}</Text></View><Text style={styles.count}>{mode === "Archive" ? archived.length : items.length - archived.length} ASSETS</Text></View>

        {mode === "Explore" ? (
          <>
            <View style={styles.search}><Text style={styles.searchIcon}>⌕</Text><TextInput value={query} onChangeText={setQuery} placeholder="Search instruments or symbols" placeholderTextColor="#A4AFBD" style={styles.searchInput} /></View>
            <View style={styles.filters}>{["All", "Crypto", "Equities", "FX"].map((name) => <Pressable key={name} onPress={() => setMarket(name)} style={[styles.filter, market === name && styles.filterActive]}><Text style={[styles.filterText, market === name && styles.filterTextActive]}>{name}</Text></Pressable>)}</View>
          </>
        ) : null}

        {data.length ? data.map((item) => (
          <View key={item.symbol} style={styles.card}>
            <Pressable onPress={() => setSelected(item)} style={styles.cardMain}>
              <View style={[styles.assetMark, { backgroundColor: item.tint }]}><Text style={styles.assetMarkText}>{item.symbol.slice(0, 1)}</Text></View>
              <View style={styles.assetCopy}><Text style={styles.assetName}>{item.name}</Text><Text style={styles.assetMeta}>{item.symbol} · {item.market}</Text></View>
              <Text style={styles.arrow}>↗</Text>
            </Pressable>
            <Text style={styles.detail}>{item.detail}</Text>
            {mode === "Explore" ? <Pressable onPress={() => toggle(item.symbol)} style={styles.addButton}><Text style={styles.addText}>＋ Add to archive</Text></Pressable> : null}
          </View>
        )) : <View style={styles.empty}><Text style={styles.emptyTitle}>Nothing here yet.</Text><Text style={styles.emptyCopy}>{mode === "Archive" ? "Explore instruments and add the ones you want to follow." : "Try another name or market."}</Text></View>}

        {mode === "Explore" ? <View style={styles.note}><Text style={styles.noteTitle}>Discovery feed</Text><Text style={styles.noteCopy}>News, macro events and instrument universes will appear when their data sources are connected. No simulated headlines are shown.</Text></View> : null}
        <Text style={styles.footer}>QOIN · RESEARCH MODE · NO LIVE EXECUTION</Text>
      </ScrollView>

      <Modal visible={!!selected} transparent animationType="slide" onRequestClose={() => setSelected(null)}>
        {selected ? <View style={styles.modalShade}><View style={styles.modalCard}>
          <View style={styles.modalHeader}><View><Text style={styles.kicker}>INSTRUMENT SPACE</Text><Text style={styles.modalTitle}>{selected.name}</Text><Text style={styles.assetMeta}>{selected.symbol} · {selected.market}</Text></View><Pressable onPress={() => setSelected(null)} style={styles.close}><Text style={styles.closeText}>×</Text></Pressable></View>
          <ScrollView contentContainerStyle={styles.modalContent}>
            <Text style={styles.mapTitle}>Force map</Text><Text style={styles.mapCopy}>A preview of interacting market dimensions.</Text>
            <View style={styles.mapWrap}>
              <Svg viewBox="0 0 100 100" width="100%" height={270}>
                <Rect x="0" y="0" width="100" height="100" fill="#FBFCFF" />
                {Array.from({ length: 9 }, (_, i) => <Line key={`v${i}`} x1={i * 12.5} y1="0" x2={i * 12.5} y2="100" stroke="#E9EEF6" strokeWidth=".4" />)}
                {Array.from({ length: 9 }, (_, i) => <Line key={`h${i}`} x1="0" y1={i * 12.5} x2="100" y2={i * 12.5} stroke="#E9EEF6" strokeWidth=".4" />)}
                <Line x1="50" y1="4" x2="50" y2="96" stroke="#CBD5E1" strokeDasharray="2 2" strokeWidth=".6" /><Line x1="4" y1="50" x2="96" y2="50" stroke="#CBD5E1" strokeDasharray="2 2" strokeWidth=".6" />
                {([{ k: "flow", c: "#2867DC", x: 50 + selected.vector.flow * 34, y: 50 - selected.vector.flow * 22 }, { k: "liquidity", c: "#60A5FA", x: 50 + selected.vector.liquidity * 28, y: 50 + selected.vector.liquidity * 27 }, { k: "volatility", c: "#8B5CF6", x: 50 - selected.vector.volatility * 32, y: 50 - selected.vector.volatility * 26 }, { k: "positioning", c: "#0F9F91", x: 50 + selected.vector.positioning * 35, y: 50 - selected.vector.positioning * 30 }] as const).map((p) => <React.Fragment key={p.k}><Line x1="50" y1="50" x2={p.x} y2={p.y} stroke={p.c} strokeWidth="1.1" strokeDasharray="2 1.5" /><Circle cx={p.x} cy={p.y} r="2.7" fill={p.c} stroke="#FFFFFF" strokeWidth="1" /></React.Fragment>)}
                <Circle cx="50" cy="50" r="5" fill={selected.tint} stroke="#FFFFFF" strokeWidth="1.5" />
              </Svg>
            </View>
            <View style={styles.legend}>{[["#2867DC", "Order flow"], ["#60A5FA", "Liquidity"], ["#8B5CF6", "Volatility"], ["#0F9F91", "Positioning"]].map(([color, label]) => <View key={label} style={styles.legendItem}><View style={[styles.legendDot, { backgroundColor: color }]} /><Text style={styles.legendText}>{label}</Text></View>)}</View>
            {Object.entries(selected.vector).map(([key, value]) => <View key={key} style={styles.metric}><Text style={styles.metricLabel}>{key.replace("_", " ")}</Text><Text style={styles.metricValue}>{value > 0 ? "+" : ""}{value.toFixed(2)}</Text></View>)}
            <Text style={styles.disclaimer}>Illustrative coordinates only. Live observations and engine-produced embeddings are not connected to this preview. Not a trading recommendation.</Text>
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
  filters: { flexDirection: "row", gap: 7, marginBottom: 14 },
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
  addButton: { marginTop: 12, alignSelf: "flex-start", paddingVertical: 5 },
  addText: { color: "#2867D8", fontSize: 10, fontWeight: "700" },
  empty: { padding: 28, borderWidth: 1, borderStyle: "dashed", borderColor: "#DCE4EF", borderRadius: 14, alignItems: "center" },
  emptyTitle: { color: "#34465F", fontSize: 13, fontWeight: "700" },
  emptyCopy: { color: "#8996A8", fontSize: 10, textAlign: "center", marginTop: 6 },
  note: { marginTop: 15, padding: 14, borderWidth: 1, borderColor: "#E8EDF4", borderRadius: 12, backgroundColor: "#FBFCFE" },
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
  disclaimer: { color: "#8996A7", fontSize: 9, lineHeight: 14, padding: 11, backgroundColor: "#F7F9FC", borderRadius: 9, marginTop: 15 },
  modalActions: { flexDirection: "row", justifyContent: "flex-end", gap: 8, padding: 14, borderTopWidth: 1, borderTopColor: "#EDF1F6" },
  closeAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#F0F3F8" },
  closeActionText: { color: "#64748B", fontSize: 11, fontWeight: "700" },
  saveAction: { paddingVertical: 10, paddingHorizontal: 14, borderRadius: 9, backgroundColor: "#2867DC" },
  saveActionText: { color: "#FFFFFF", fontSize: 11, fontWeight: "700" },
});
