import React from "react";
import { View, Text, StyleSheet } from "react-native";
import { colors } from "../theme";

/**
 * Deliberately read-only until an authenticated, risk-checked execution API
 * and exchange reconciliation are available. Never present a local UI state
 * change as a successfully filled exchange order.
 */
export function OrderTicket({ symbol }: { symbol: string }) {
  return (
    <View style={styles.card} accessibilityRole="summary">
      <Text style={styles.title}>Trading unavailable</Text>
      <Text style={styles.symbol}>{symbol}</Text>
      <Text style={styles.message}>
        Live order execution is not connected. No order has been submitted and
        no funds have been moved. Market research and backtesting remain available.
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  card: {
    backgroundColor: colors.bgSecondary,
    borderRadius: 12,
    padding: 14,
    marginTop: 12,
  },
  title: {
    color: colors.textPrimary,
    fontSize: 13,
    fontWeight: "700",
  },
  symbol: {
    color: colors.textMuted,
    fontSize: 12,
    marginTop: 4,
  },
  message: {
    color: colors.textMuted,
    fontSize: 12,
    lineHeight: 18,
    marginTop: 8,
  },
});
