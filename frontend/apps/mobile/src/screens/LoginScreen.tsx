import React, { useState } from "react";
import { View, Text, TextInput, Pressable, StyleSheet, ActivityIndicator } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { signInQoinUser, registerQoinUser } from "@qoin/core";
import { colors } from "../theme";
import type { RootStackParamList } from "../../App";

type Props = NativeStackScreenProps<RootStackParamList, "Login"> & { configurationError?: string };

export function LoginScreen({ navigation, configurationError }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [register, setRegister] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async () => {
    setError("");
    if (!email.trim() || !password) { setError("Enter your email and password."); return; }
    setBusy(true);
    try {
      if (register) await registerQoinUser(email, password);
      else await signInQoinUser(email, password);
      navigation.replace("Dashboard");
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : "Authentication failed.");
    } finally { setBusy(false); }
  };

  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.card}>
        <Text style={styles.brand}>Qoin</Text>
        <Text style={styles.subtitle}>A transparent market research workspace</Text>
        <Text style={styles.label}>Email</Text>
        <TextInput style={styles.input} value={email} onChangeText={setEmail} autoCapitalize="none"
          autoComplete="email" keyboardType="email-address" placeholder="you@example.com" placeholderTextColor={colors.textMuted} />
        <Text style={styles.label}>Password</Text>
        <TextInput style={styles.input} value={password} onChangeText={setPassword} secureTextEntry
          autoComplete={register ? "new-password" : "current-password"} placeholder="Password" placeholderTextColor={colors.textMuted} />
        {configurationError ? <Text style={styles.error}>{configurationError}</Text> : null}
        {error ? <Text style={styles.error}>{error}</Text> : null}
        <Pressable style={[styles.button, (busy || Boolean(configurationError)) && styles.disabled]} disabled={busy || Boolean(configurationError)} onPress={submit}>
          {busy ? <ActivityIndicator color="#FFFFFF" /> : <Text style={styles.buttonText}>{register ? "Create account" : "Sign in"}</Text>}
        </Pressable>
        <Pressable style={styles.switch} onPress={() => { setRegister((value) => !value); setError(""); }}>
          <Text style={styles.switchText}>{register ? "Already have an account? Sign in" : "New to Qoin? Create an account"}</Text>
        </Pressable>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, justifyContent: "center", padding: 20 },
  card: { backgroundColor: colors.bgSecondary, borderRadius: 20, padding: 22 },
  brand: { color: colors.textPrimary, fontSize: 28, fontWeight: "800", textAlign: "center" },
  subtitle: { color: colors.textMuted, fontSize: 12, textAlign: "center", marginTop: 6, marginBottom: 18 },
  label: { color: colors.textMuted, fontSize: 12, marginTop: 12, marginBottom: 5 },
  input: { backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 12, padding: 12, color: colors.textPrimary },
  button: { backgroundColor: colors.accentBlue, borderRadius: 14, padding: 13, alignItems: "center", marginTop: 20 },
  disabled: { opacity: 0.45 },
  buttonText: { color: "#FFFFFF", fontWeight: "700" },
  switch: { alignItems: "center", padding: 14 },
  switchText: { color: colors.accentBlue, fontSize: 12, fontWeight: "700" },
  error: { color: "#C24141", fontSize: 11, marginTop: 12 },
});
