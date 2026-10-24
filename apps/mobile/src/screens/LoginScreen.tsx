import React, { useState } from "react";
import { View, Text, TextInput, Pressable, StyleSheet } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { colors } from "../theme";
import type { RootStackParamList } from "../../App";

type Props = NativeStackScreenProps<RootStackParamList, "Login">;

export function LoginScreen({ navigation }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");

  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.card}>
        <Text style={styles.brand}>Qoin</Text>
        <Text style={styles.label}>Email</Text>
        <TextInput
          style={styles.input}
          value={email}
          onChangeText={setEmail}
          autoCapitalize="none"
          keyboardType="email-address"
          placeholderTextColor={colors.textMuted}
        />
        <Text style={styles.label}>Password</Text>
        <TextInput
          style={styles.input}
          value={password}
          onChangeText={setPassword}
          secureTextEntry
          placeholderTextColor={colors.textMuted}
        />
        <Pressable style={styles.button} onPress={() => navigation.replace("Dashboard")}>
          <Text style={styles.buttonText}>Sign in</Text>
        </Pressable>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, justifyContent: "center", padding: 20 },
  card: { backgroundColor: colors.bgSecondary, borderRadius: 12, borderWidth: 1, borderColor: colors.border, padding: 20 },
  brand: { color: colors.white, fontSize: 24, fontWeight: "700", textAlign: "center", marginBottom: 24 },
  label: { color: colors.textMuted, fontSize: 12, marginTop: 12, marginBottom: 4 },
  input: { backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 8, padding: 10, color: colors.textPrimary },
  button: { backgroundColor: colors.accentCyan, borderRadius: 8, padding: 12, alignItems: "center", marginTop: 20 },
  buttonText: { color: colors.bgPrimary, fontWeight: "700" },
});
