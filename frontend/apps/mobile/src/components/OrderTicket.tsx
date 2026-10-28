import React, { useState } from "react";
import { View, Text, TextInput, Pressable, StyleSheet } from "react-native";
import type { Side, TickerQuote } from "@qoin/core";
import { colors } from "../theme";

export function OrderTicket({ symbol, quote }: { symbol: string; quote: TickerQuote }) {
  const [side, setSide] = useState<Side>("buy");
  const [size, setSize] = useState("");
  const [confirming, setConfirming] = useState(false);
  const [placed, setPlaced] = useState<string | null>(null);

  const sizeNum = parseFloat(size) || 0;
  const estimate = sizeNum * quote.price;
  const valid = sizeNum > 0;

  function confirm() {
    setConfirming(false);
    setPlaced(`${side === "buy" ? "Bought" : "Sold"} ${size} ${symbol} at market`);
    setSize("");
    setTimeout(() => setPlaced(null), 4000);
  }

  return (
    <View style={styles.card}>
      <View style={styles.sideRow}>
        {(["buy", "sell"] as Side[]).map((s) => (
          <Pressable
            key={s}
            onPress={() => setSide(s)}
            style={[
              styles.sideBtn,
              side === s && { backgroundColor: s === "buy" ? "#22C55E26" : "#EF444426" },
            ]}
          >
            <Text
              style={{
                color: side === s ? (s === "buy" ? colors.bullish : colors.bearish) : colors.textMuted,
                fontWeight: "700",
                fontSize: 12,
                textTransform: "uppercase",
              }}
            >
              {s}
            </Text>
          </Pressable>
        ))}
      </View>

      <Text style={styles.label}>Size ({symbol.split("/")[0]})</Text>
      <TextInput
        style={styles.input}
        value={size}
        onChangeText={setSize}
        keyboardType="decimal-pad"
        placeholder="0.00"
        placeholderTextColor={colors.textMuted}
      />

      <View style={styles.estRow}>
        <Text style={{ color: colors.textMuted, fontSize: 12 }}>Est. total</Text>
        <Text style={{ color: colors.textPrimary, fontSize: 12 }}>${estimate.toFixed(2)}</Text>
      </View>

      {!confirming ? (
        <Pressable
          disabled={!valid}
          onPress={() => setConfirming(true)}
          style={[
            styles.submit,
            { backgroundColor: side === "buy" ? colors.bullish : colors.bearish, opacity: valid ? 1 : 0.4 },
          ]}
        >
          <Text style={{ color: colors.bgPrimary, fontWeight: "700" }}>
            {side === "buy" ? "Buy" : "Sell"} {symbol}
          </Text>
        </Pressable>
      ) : (
        <View style={styles.confirmBox}>
          <Text style={{ color: colors.textMuted, fontSize: 12, marginBottom: 8 }}>
            Confirm: {side} {size} {symbol} at market?
          </Text>
          <View style={{ flexDirection: "row", gap: 8 }}>
            <Pressable onPress={confirm} style={[styles.confirmBtn, { backgroundColor: colors.accentCyan }]}>
              <Text style={{ color: colors.bgPrimary, fontWeight: "700" }}>Confirm</Text>
            </Pressable>
            <Pressable onPress={() => setConfirming(false)} style={[styles.confirmBtn, { borderWidth: 1, borderColor: colors.border }]}>
              <Text style={{ color: colors.textPrimary, fontWeight: "600" }}>Cancel</Text>
            </Pressable>
          </View>
        </View>
      )}

      {placed && <Text style={{ color: colors.bullish, fontSize: 12, marginTop: 8 }}>{placed}</Text>}
    </View>
  );
}

const styles = StyleSheet.create({
  card: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 12, padding: 14, marginTop: 12 },
  sideRow: { flexDirection: "row", borderWidth: 1, borderColor: colors.border, borderRadius: 8, overflow: "hidden", marginBottom: 10 },
  sideBtn: { flex: 1, paddingVertical: 8, alignItems: "center" },
  label: { color: colors.textMuted, fontSize: 11, marginBottom: 4 },
  input: { backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 8, padding: 10, color: colors.textPrimary, marginBottom: 10 },
  estRow: { flexDirection: "row", justifyContent: "space-between", marginBottom: 10 },
  submit: { borderRadius: 8, paddingVertical: 12, alignItems: "center" },
  confirmBox: { borderWidth: 1, borderColor: colors.border, borderRadius: 8, padding: 10 },
  confirmBtn: { flex: 1, borderRadius: 8, paddingVertical: 10, alignItems: "center" },
});
