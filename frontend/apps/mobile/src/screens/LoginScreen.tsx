import React, { useState } from "react";
import { View, Text as NativeText, type TextProps, TextInput as NativeTextInput, type TextInputProps, Pressable, StyleSheet, ActivityIndicator } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { signInQoinUser, registerQoinUser } from "@qoin/core";
import { colors } from "../theme";
import type { RootStackParamList } from "../../App";

type Props = NativeStackScreenProps<RootStackParamList, "Login"> & { configurationError?: string };

function Text(props: TextProps) {
  const flattened = StyleSheet.flatten(props.style);
  const weight = flattened?.fontWeight;
  const fontFamily = flattened?.fontFamily ?? (
    weight === "700" || weight === "800" || weight === "bold"
      ? "DMSans_700Bold"
      : weight === "500" || weight === "600"
        ? "DMSans_500Medium"
        : "DMSans_400Regular"
  );
  return <NativeText {...props} style={[{ fontFamily }, props.style]} />;
}

function TextInput(props: TextInputProps) {
  return <NativeTextInput {...props} selectionColor={props.selectionColor ?? "#7DF162"} style={[{ fontFamily: "DMSans_400Regular" }, props.style]} />;
}


export function LoginScreen({ configurationError, route }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [register, setRegister] = useState(route.params?.mode === "register");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const submit = async () => {
    setError("");
    if (!email.trim() || !password) { setError("Enter your email and password."); return; }
    setBusy(true);
    try {
      if (register) await registerQoinUser(email, password);
      else await signInQoinUser(email, password);
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
          {busy ? <ActivityIndicator color="#061308" /> : <Text style={styles.buttonText}>{register ? "Create account" : "Sign in"}</Text>}
        </Pressable>
        <Pressable style={styles.switch} onPress={() => { setRegister((value) => !value); setError(""); }}>
          <Text style={styles.switchText}>{register ? "Already have an account? Sign in" : "New to Qoin? Create an account"}</Text>
        </Pressable>
        <Text style={styles.footnote}>Sign in to sync your Archive and account preferences across devices.</Text>
      </View>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1, backgroundColor: colors.bgPrimary, justifyContent: "center", padding: 20 },
  card: { backgroundColor: colors.bgSecondary, borderRadius: 12, padding: 22 },
  brand: { color: colors.accent, fontSize: 28, fontWeight: "800", textAlign: "center" },
  subtitle: { color: colors.textMuted, fontSize: 12, textAlign: "center", marginTop: 6, marginBottom: 18 },
  label: { color: colors.textMuted, fontSize: 12, marginTop: 12, marginBottom: 5 },
  input: { backgroundColor: colors.bgPrimary, borderWidth: 0, borderColor: "transparent", borderRadius: 8, padding: 12, color: colors.textPrimary },
  button: { backgroundColor: colors.accentCyan, borderRadius: 8, padding: 13, alignItems: "center", marginTop: 20 },
  disabled: { opacity: 0.45 },
  buttonText: { color: "#061308", fontWeight: "700" },
  switch: { alignItems: "center", padding: 14 },
  switchText: { color: colors.accentCyan, fontSize: 12, fontWeight: "700" },
  footnote: { color: colors.textMuted, fontSize: 10, textAlign: "center", lineHeight: 16, marginTop: 8 },
  error: { color: "#C24141", fontSize: 11, marginTop: 12 },
});
