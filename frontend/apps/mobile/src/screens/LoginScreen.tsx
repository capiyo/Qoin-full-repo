import React from "react";
import { View, Text, Pressable, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { colors } from "../theme";
import type { RootStackParamList } from "../../App";

type Props = NativeStackScreenProps<RootStackParamList, "Login">;

export function LoginScreen({ navigation }: Props) {
  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.card}>
        <View style={styles.brandMark}><Text style={styles.brandMarkText}>Q</Text></View>
        <Text style={styles.brand}>QOIN</Text>
        <Text style={styles.eyebrow}>RESEARCH PREVIEW</Text>
        <Text style={styles.title}>Explore your market universe.</Text>
        <Text style={styles.copy}>
          Account sign-in is not available yet. Qoin has not been connected to its own verified identity provider, so this preview will not collect credentials or pretend to authenticate you.
        </Text>
        <View style={styles.notice}>
          <Text style={styles.noticeTitle}>Preview limitations</Text>
          <Text style={styles.noticeCopy}>Your basket stays on this device. Account synchronization and personalized portfolio storage are unavailable. No live orders can be placed.</Text>
        </View>
        <Pressable
          accessibilityRole="button"
          style={styles.button}
          onPress={() => navigation.replace("Dashboard")}
        >
          <Text style={styles.buttonText}>Continue in research preview</Text>
        </Pressable>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, justifyContent: "center", padding: 20 },
  card: { backgroundColor: colors.bgSecondary, borderRadius: 16, borderWidth: 1, borderColor: colors.border, padding: 22 },
  brandMark: { width: 42, height: 42, borderRadius: 22, backgroundColor: colors.accentCyan, alignItems: "center", justifyContent: "center", alignSelf: "center", marginBottom: 10 },
  brandMarkText: { color: colors.bgPrimary, fontWeight: "900", fontSize: 22 },
  brand: { color: colors.white, fontSize: 22, fontWeight: "900", textAlign: "center", letterSpacing: 3, marginBottom: 28 },
  eyebrow: { color: colors.accentCyan, fontSize: 10, fontWeight: "800", letterSpacing: 1.6, marginBottom: 8 },
  title: { color: colors.white, fontSize: 23, lineHeight: 29, fontWeight: "800", marginBottom: 10 },
  copy: { color: colors.textMuted, fontSize: 13, lineHeight: 20 },
  notice: { backgroundColor: colors.bgPrimary, borderRadius: 10, padding: 14, marginTop: 20, borderWidth: 1, borderColor: colors.border },
  noticeTitle: { color: colors.textPrimary, fontSize: 12, fontWeight: "800", marginBottom: 5 },
  noticeCopy: { color: colors.textMuted, fontSize: 11, lineHeight: 17 },
  button: { backgroundColor: colors.accentCyan, borderRadius: 10, padding: 14, alignItems: "center", marginTop: 22 },
  buttonText: { color: colors.bgPrimary, fontWeight: "800", fontSize: 13 },
});
