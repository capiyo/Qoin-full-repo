import { View, Text, StyleSheet } from "react-native";
import type { FeedStatus } from "@qoin/core";
import { colors } from "../theme";

const CONFIG: Record<FeedStatus, { label: string; color: string }> = {
  connecting: { label: "Connecting", color: colors.warning },
  live: { label: "Live", color: colors.bullish },
  stale: { label: "Stale data", color: colors.warning },
  disconnected: { label: "Disconnected", color: colors.bearish },
};

export function StatusBadge({ status }: { status: FeedStatus }) {
  const c = CONFIG[status];
  return (
    <View style={styles.row}>
      <View style={[styles.dot, { backgroundColor: c.color }]} />
      <Text style={[styles.label, { color: c.color }]}>{c.label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  row: { flexDirection: "row", alignItems: "center", gap: 6 },
  dot: { width: 6, height: 6, borderRadius: 3 },
  label: { fontSize: 12, fontWeight: "600" },
});
