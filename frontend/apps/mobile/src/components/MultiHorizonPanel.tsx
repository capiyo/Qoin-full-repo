import React, { useEffect, useState } from "react";
import { ActivityIndicator, Pressable, ScrollView, StyleSheet, Text, TextInput, View } from "react-native";
import { getQoinMarketHorizons, type QoinMarketHorizonsResponse } from "@qoin/core";

type InstrumentOption = {
  symbol: string; name: string; market: string; engineSymbol: string; venue: string; engineVenue: string;
};
type Props = { instruments: InstrumentOption[] };
const palette = { panel: "#14231B", border: "#244B33", accent: "#9DE4B0", text: "#EDF4EE", muted: "#9BAFA0", waiting: "#D9A85B" };
const keyOf = (item: InstrumentOption) => item.engineSymbol + "::" + item.engineVenue;

export function MultiHorizonPanel({ instruments }: Props) {
  const available = instruments.filter((item) => item.engineSymbol.trim() && item.engineVenue.trim());
  const [selectedKey, setSelectedKey] = useState("");
  const [minEvents, setMinEvents] = useState("20");
  const [result, setResult] = useState<QoinMarketHorizonsResponse | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    if (!available.length) { setSelectedKey(""); setResult(null); return; }
    if (!available.some((item) => keyOf(item) === selectedKey)) {
      setSelectedKey(keyOf(available[0]));
      setResult(null);
    }
  }, [instruments, selectedKey]);

  const selected = available.find((item) => keyOf(item) === selectedKey);
  const load = async () => {
    if (!selected) return;
    const minimum = Number(minEvents);
    if (!Number.isInteger(minimum) || minimum < 5 || minimum > 10_000) {
      setError("Minimum events must be between 5 and 10,000.");
      return;
    }
    setBusy(true); setError("");
    try { setResult(await getQoinMarketHorizons(selected.engineSymbol, selected.engineVenue, minimum)); }
    catch (cause) { setError(cause instanceof Error ? cause.message : "Multi-horizon research is unavailable."); setResult(null); }
    finally { setBusy(false); }
  };

  return (
    <View style={styles.panel}>
      <View style={styles.heading}>
        <View style={styles.headingCopy}>
          <Text style={styles.eyebrow}>CAUSAL RESEARCH · NO ORDER AUTHORITY</Text>
          <Text style={styles.title}>Multi-horizon research</Text>
          <Text style={styles.caption}>Independent 1m to 1w windows for one exact venue and instrument</Text>
        </View>
        <Text style={styles.badge}>1m → 1w</Text>
      </View>
      <ScrollView horizontal showsHorizontalScrollIndicator={false} contentContainerStyle={styles.instrumentList}>
        {available.map((item) => (
          <Pressable key={keyOf(item)} onPress={() => { setSelectedKey(keyOf(item)); setResult(null); }} style={[styles.instrumentChip, selectedKey === keyOf(item) && styles.instrumentChipActive]}>
            <Text style={[styles.instrumentName, selectedKey === keyOf(item) && styles.instrumentNameActive]}>{item.name}</Text>
            <Text style={styles.instrumentMeta}>{item.venue}</Text>
          </Pressable>
        ))}
      </ScrollView>
      <View style={styles.controls}>
        <View style={styles.inputGroup}>
          <Text style={styles.inputLabel}>Minimum events per horizon</Text>
          <TextInput value={minEvents} onChangeText={setMinEvents} keyboardType="number-pad" style={styles.input} />
        </View>
        <Pressable onPress={load} disabled={busy || !selected} style={[styles.loadButton, (busy || !selected) && styles.disabled]}>
          {busy ? <ActivityIndicator color={palette.panel} /> : <Text style={styles.loadText}>Load horizons</Text>}
        </Pressable>
      </View>
      {error ? <Text style={styles.error}>{error}</Text> : null}
      {!available.length ? <Text style={styles.empty}>The canonical instrument registry is unavailable. No fallback instruments are substituted.</Text> : null}
      {result ? (
        <>
          <View style={styles.summary}>
            <Text style={styles.summaryText}>{result.symbol} · {result.venue_id}</Text>
            <Text style={styles.summaryText}>{result.events_loaded.toLocaleString()} events · research only</Text>
          </View>
          <View style={styles.grid}>
            {result.observations.map((item) => {
              const signal = result.signals?.horizons.find((entry) => entry.horizon === item.horizon)?.signal;
              return (
                <View key={item.horizon} style={styles.card}>
                  <View style={styles.cardHeader}>
                    <Text style={styles.cardTitle}>{item.horizon.replaceAll("_", " ")}</Text>
                    <Text style={[styles.ready, { color: item.ready ? palette.accent : palette.waiting }]}>{item.ready ? "READY" : "WAITING"}</Text>
                  </View>
                  <Text style={styles.signal}>{signal?.decision ?? "—"}</Text>
                  <Text style={styles.meta}>{item.event_count} events · {item.trade_count} trades · {item.book_count} books</Text>
                  <Text style={styles.meta}>Coverage {(item.coverage_ratio * 100).toFixed(1)}% · Quality {(item.data_quality * 100).toFixed(0)}%</Text>
                  <Text style={styles.meta}>OFI {item.observation.ofi.toFixed(2)} · OI Δ {item.observation.open_interest_change.toFixed(3)}</Text>
                  {!item.ready ? <Text style={styles.reason}>{item.reasons.slice(0, 2).join("; ")}</Text> : null}
                  {signal ? <Text style={styles.meta}>Score {signal.score.toFixed(3)} · Alignment {signal.alignment.toFixed(3)}</Text> : null}
                </View>
              );
            })}
          </View>
          {result.signals ? <Text style={styles.consensus}>Horizon relationship: {result.signals.consensus.relationship.replaceAll("_", " ")} · BUY {result.signals.consensus.buy_count} · SELL {result.signals.consensus.sell_count} · WAIT {result.signals.consensus.wait_count}</Text> : null}
          {result.event_limit_reached || result.invalid_event_documents > 0 ? <Text style={styles.error}>Signals suppressed by the event safety cap or invalid stored event documents.</Text> : null}
          <Text style={styles.caption}>{result.note}</Text>
        </>
      ) : <Text style={styles.empty}>Load stored market events. Insufficient trade or book evidence produces WAITING, not a fabricated signal.</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  panel: { marginTop: 14, padding: 14, borderRadius: 18, borderWidth: 1, borderColor: palette.border, backgroundColor: palette.panel, gap: 12 },
  heading: { flexDirection: "row", alignItems: "flex-start", justifyContent: "space-between", gap: 10 },
  headingCopy: { flex: 1, gap: 4 },
  eyebrow: { color: palette.accent, fontSize: 9, fontWeight: "800", letterSpacing: 1 },
  title: { color: palette.text, fontSize: 16, fontWeight: "800" },
  caption: { color: palette.muted, fontSize: 10, lineHeight: 15 },
  badge: { color: palette.accent, fontSize: 10, fontWeight: "800" },
  instrumentList: { gap: 8, paddingVertical: 2 },
  instrumentChip: { paddingVertical: 8, paddingHorizontal: 11, borderRadius: 12, borderWidth: 1, borderColor: palette.border, backgroundColor: "#0E1912", gap: 3 },
  instrumentChipActive: { borderColor: palette.accent, backgroundColor: "#203B2A" },
  instrumentName: { color: palette.text, fontSize: 10, fontWeight: "700" },
  instrumentNameActive: { color: palette.accent },
  instrumentMeta: { color: palette.muted, fontSize: 8 },
  controls: { flexDirection: "row", alignItems: "flex-end", gap: 9 },
  inputGroup: { flex: 1, gap: 5 },
  inputLabel: { color: palette.muted, fontSize: 9, fontWeight: "700" },
  input: { minHeight: 38, borderRadius: 10, borderWidth: 1, borderColor: palette.border, paddingHorizontal: 10, color: palette.text, backgroundColor: "#0E1912", fontSize: 12 },
  loadButton: { minHeight: 38, borderRadius: 999, backgroundColor: palette.accent, paddingHorizontal: 14, justifyContent: "center", alignItems: "center" },
  loadText: { color: palette.panel, fontSize: 10, fontWeight: "800" },
  disabled: { opacity: 0.5 },
  error: { color: palette.waiting, fontSize: 10, lineHeight: 15 },
  empty: { color: palette.muted, fontSize: 10, lineHeight: 16 },
  summary: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  summaryText: { color: palette.accent, fontSize: 9, fontWeight: "700" },
  grid: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  card: { width: "48%", minWidth: 140, flexGrow: 1, borderRadius: 12, borderWidth: 1, borderColor: palette.border, backgroundColor: "#0E1912", padding: 10, gap: 5 },
  cardHeader: { flexDirection: "row", justifyContent: "space-between", gap: 4 },
  cardTitle: { color: palette.text, fontSize: 10, fontWeight: "800", textTransform: "capitalize" },
  ready: { fontSize: 8, fontWeight: "900" },
  signal: { color: palette.text, fontSize: 17, fontWeight: "900", marginTop: 3 },
  meta: { color: palette.muted, fontSize: 8, lineHeight: 12 },
  reason: { color: palette.waiting, fontSize: 8, lineHeight: 12 },
  consensus: { color: palette.text, fontSize: 10, lineHeight: 15, fontWeight: "700" },
});
