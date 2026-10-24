import React, { useEffect, useMemo, useState } from "react";
import { ScrollView, View, Text, TextInput, Pressable, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import {
  evaluateQoinSignal,
  getQoinHealth,
  type MarketForceObservation,
  type QoinSignal,
} from "@qoin/core";
import { colors } from "../theme";

const initial: MarketForceObservation = {
  ofi: 1.2,
  liquidity: 0.4,
  volatility: 0.3,
  funding: -0.2,
  open_interest_change: 0.6,
  liquidation_pressure: 0.1,
  basis: 0,
};

const fields: Array<{ key: keyof MarketForceObservation; label: string; help: string }> = [
  { key: "ofi", label: "Order-flow imbalance", help: "Aggressive buy/sell pressure" },
  { key: "liquidity", label: "Liquidity / depth", help: "Normalized liquidity change" },
  { key: "volatility", label: "Volatility", help: "Context and directional dampening" },
  { key: "funding", label: "Funding", help: "Perpetual positioning pressure" },
  { key: "open_interest_change", label: "Open interest change", help: "Change in open positions" },
  { key: "liquidation_pressure", label: "Liquidation pressure", help: "Forced-flow intensity" },
  { key: "basis", label: "Basis", help: "Perpetual / spot dislocation" },
];

export function SignalScreen() {
  const [forces, setForces] = useState(initial);
  const [signal, setSignal] = useState<QoinSignal | null>(null);
  const [apiOnline, setApiOnline] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  useEffect(() => {
    getQoinHealth().then((health) => setApiOnline(health.status === "ok" && !health.live_trading))
      .catch(() => setApiOnline(false));
  }, []);

  const activeCount = useMemo(
    () => fields.filter(({ key }) => Math.abs(forces[key]) > 0.0001).length,
    [forces],
  );

  async function evaluate() {
    setBusy(true);
    setError("");
    try {
      const result = await evaluateQoinSignal({ observation: forces, has_open_position: false });
      setSignal(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not reach the Qoin API.");
    } finally {
      setBusy(false);
    }
  }

  const decisionColor = signal?.decision === "BUY" ? colors.bullish
    : signal?.decision === "SELL" ? colors.bearish : colors.accentCyan;

  return (
    <SafeAreaView style={styles.screen}>
      <ScrollView contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        <View style={styles.header}>
          <View>
            <Text style={styles.kicker}>QOIN / RESEARCH ENGINE</Text>
            <Text style={styles.title}>Force vectors</Text>
          </View>
          <View style={styles.statusPill}>
            <View style={[styles.dot, { backgroundColor: apiOnline ? colors.bullish : colors.textMuted }]} />
            <Text style={styles.statusText}>{apiOnline ? "API online" : "API offline"}</Text>
          </View>
        </View>
        <Text style={styles.subtitle}>Explore current normalized forces and evaluate a versioned signal. This screen never submits orders.</Text>

        <View style={styles.panel}>
          <View style={styles.panelHeader}>
            <Text style={styles.panelTitle}>Observation vector</Text>
            <Text style={styles.counter}>{activeCount}/7 active</Text>
          </View>
          {fields.map(({ key, label, help }) => (
            <View key={key} style={styles.fieldRow}>
              <View style={styles.fieldCopy}>
                <Text style={styles.fieldLabel}>{label}</Text>
                <Text style={styles.fieldHelp}>{help}</Text>
              </View>
              <TextInput
                accessibilityLabel={label}
                keyboardType="decimal-pad"
                value={String(forces[key])}
                onChangeText={(value) => {
                  const parsed = Number(value);
                  if (value.trim() !== "" && Number.isFinite(parsed)) {
                    setForces((current) => ({ ...current, [key]: parsed }));
                  } else if (value.trim() === "" || value === "-") {
                    setForces((current) => ({ ...current, [key]: 0 }));
                  }
                }}
                style={styles.input}
                selectTextOnFocus
              />
            </View>
          ))}
          <Pressable onPress={evaluate} disabled={busy} style={[styles.button, busy && styles.disabled]}>
            <Text style={styles.buttonText}>{busy ? "Evaluating…" : "Evaluate research signal"}</Text>
          </Pressable>
          {error ? <Text accessibilityRole="alert" style={styles.error}>{error}</Text> : null}
        </View>

        <View style={styles.panel}>
          <Text style={styles.kicker}>VECTOR CONVERGENCE</Text>
          <Text style={styles.panelTitle}>Signal output</Text>
          {signal ? (
            <>
              <Text style={[styles.decision, { color: decisionColor }]}>{signal.decision}</Text>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Directional score</Text><Text style={styles.metricValue}>{signal.score.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Angular alignment</Text><Text style={styles.metricValue}>{signal.alignment.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Angle (radians)</Text><Text style={styles.metricValue}>{signal.angle_radians.toFixed(3)}</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Active forces</Text><Text style={styles.metricValue}>{signal.active_force_count}/7</Text></View>
              <View style={styles.metricRow}><Text style={styles.metricLabel}>Config version</Text><Text style={styles.metricValue}>{signal.config_version}</Text></View>
              <Text style={styles.rationale}>{signal.rationale}</Text>
            </>
          ) : (
            <Text style={styles.empty}>No signal evaluated yet. Inputs are manually supplied normalized observations, not a live exchange feed.</Text>
          )}
        </View>
        <View style={styles.safety}>
          <Text style={styles.safetyTitle}>Research only · no live execution</Text>
          <Text style={styles.safetyBody}>The baseline is not calibrated and has no demonstrated profitability. Signals do not size positions, pass a risk gate, or place orders.</Text>
        </View>
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary },
  content: { padding: 16, paddingBottom: 28 },
  header: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", gap: 12, marginBottom: 12 },
  kicker: { color: colors.accentCyan, fontSize: 10, fontWeight: "800", letterSpacing: 1.5, marginBottom: 6 },
  title: { color: colors.white, fontSize: 25, fontWeight: "800" },
  subtitle: { color: colors.textMuted, fontSize: 13, lineHeight: 19, marginBottom: 16 },
  statusPill: { flexDirection: "row", alignItems: "center", gap: 6, paddingHorizontal: 9, paddingVertical: 7, borderRadius: 20, backgroundColor: colors.bgSecondary },
  dot: { width: 6, height: 6, borderRadius: 3 },
  statusText: { color: colors.textMuted, fontSize: 10, fontWeight: "600" },
  panel: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 14, padding: 16, marginBottom: 14 },
  panelHeader: { flexDirection: "row", justifyContent: "space-between", alignItems: "center", marginBottom: 6 },
  panelTitle: { color: colors.white, fontSize: 18, fontWeight: "700", marginBottom: 12 },
  counter: { color: colors.accentCyan, fontSize: 11, fontWeight: "700" },
  fieldRow: { flexDirection: "row", alignItems: "center", justifyContent: "space-between", gap: 10, paddingVertical: 11, borderBottomWidth: 1, borderBottomColor: colors.border },
  fieldCopy: { flex: 1 },
  fieldLabel: { color: colors.textPrimary, fontSize: 12, fontWeight: "700" },
  fieldHelp: { color: colors.textMuted, fontSize: 10, marginTop: 3 },
  input: { width: 92, color: colors.white, backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 7, paddingHorizontal: 8, paddingVertical: 8, textAlign: "right", fontSize: 13 },
  button: { backgroundColor: colors.accentCyan, borderRadius: 8, alignItems: "center", paddingVertical: 13, marginTop: 18 },
  disabled: { opacity: 0.55 },
  buttonText: { color: colors.bgPrimary, fontSize: 12, fontWeight: "800" },
  error: { color: colors.bearish, fontSize: 11, marginTop: 10 },
  decision: { fontSize: 30, fontWeight: "900", letterSpacing: 2, marginVertical: 8 },
  metricRow: { flexDirection: "row", justifyContent: "space-between", gap: 12, paddingVertical: 8, borderBottomWidth: 1, borderBottomColor: colors.border },
  metricLabel: { color: colors.textMuted, fontSize: 11 },
  metricValue: { color: colors.white, fontSize: 11, fontWeight: "700", maxWidth: "58%", textAlign: "right" },
  rationale: { color: colors.textMuted, fontSize: 11, lineHeight: 16, marginTop: 12 },
  empty: { color: colors.textMuted, fontSize: 12, lineHeight: 18, marginTop: 6 },
  safety: { borderLeftWidth: 2, borderLeftColor: colors.accentCyan, paddingLeft: 12, marginTop: 2 },
  safetyTitle: { color: colors.accentCyan, fontSize: 11, fontWeight: "800" },
  safetyBody: { color: colors.textMuted, fontSize: 11, lineHeight: 17, marginTop: 6 },
});
