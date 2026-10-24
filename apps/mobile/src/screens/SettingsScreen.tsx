import React, { useState } from "react";
import { View, Text, Switch, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import { colors } from "../theme";

export function SettingsScreen() {
  const [riskWarnings, setRiskWarnings] = useState(true);
  const [confirmOrders, setConfirmOrders] = useState(true);

  return (
    <SafeAreaView style={styles.screen}>
      <Text style={styles.title}>Settings</Text>
      <View style={styles.card}>
        <View style={styles.row}>
          <View style={{ flex: 1 }}>
            <Text style={styles.label}>Risk limit warnings</Text>
            <Text style={styles.hint}>Warn when a position nears its risk limit.</Text>
          </View>
          <Switch
            value={riskWarnings}
            onValueChange={setRiskWarnings}
            trackColor={{ false: colors.border, true: colors.accentCyan }}
            thumbColor={colors.white}
          />
        </View>
        <View style={styles.row}>
          <View style={{ flex: 1 }}>
            <Text style={styles.label}>Confirm before placing orders</Text>
            <Text style={styles.hint}>Adds a confirmation step to orders.</Text>
          </View>
          <Switch
            value={confirmOrders}
            onValueChange={setConfirmOrders}
            trackColor={{ false: colors.border, true: colors.accentCyan }}
            thumbColor={colors.white}
          />
        </View>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, padding: 16 },
  title: { color: colors.white, fontSize: 20, fontWeight: "700", marginBottom: 12 },
  card: { backgroundColor: colors.bgSecondary, borderWidth: 1, borderColor: colors.border, borderRadius: 12, padding: 16 },
  row: { flexDirection: "row", alignItems: "center", paddingVertical: 12 },
  label: { color: colors.textPrimary, fontWeight: "600", fontSize: 14 },
  hint: { color: colors.textMuted, fontSize: 12, marginTop: 2 },
});
