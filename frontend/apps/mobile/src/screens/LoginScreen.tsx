import React, { useEffect, useState } from "react";
import { View, Text, Pressable, TextInput, StyleSheet, ActivityIndicator } from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";
import type { NativeStackScreenProps } from "@react-navigation/native-stack";
import { createUserWithEmailAndPassword, onAuthStateChanged, signInWithEmailAndPassword } from "firebase/auth";
import { colors } from "../theme";
import { firebaseAuth, isFirebaseConfigured } from "../firebaseAuth";
import type { RootStackParamList } from "../../App";

type Props = NativeStackScreenProps<RootStackParamList, "Login">;

export function LoginScreen({ navigation }: Props) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [creatingAccount, setCreatingAccount] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!firebaseAuth) return undefined;
    return onAuthStateChanged(firebaseAuth, (user) => {
      if (user) navigation.replace("Dashboard");
    });
  }, [navigation]);

  async function submit() {
    if (!firebaseAuth) return;
    setError(null);
    setBusy(true);
    try {
      if (creatingAccount) {
        await createUserWithEmailAndPassword(firebaseAuth, email.trim(), password);
      } else {
        await signInWithEmailAndPassword(firebaseAuth, email.trim(), password);
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : "Authentication failed. Try again.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <SafeAreaView style={styles.screen}>
      <View style={styles.card}>
        <View style={styles.brandMark}><Text style={styles.brandMarkText}>Q</Text></View>
        <Text style={styles.brand}>QOIN</Text>
        {isFirebaseConfigured ? (
          <>
            <Text style={styles.eyebrow}>SECURE ACCOUNT</Text>
            <Text style={styles.title}>{creatingAccount ? "Create your Qoin account." : "Welcome back."}</Text>
            <Text style={styles.copy}>Sign in with Qoin's own Firebase identity project. Your account token is verified by the Qoin API before portfolio data is accessed.</Text>
            <TextInput
              autoCapitalize="none"
              autoComplete="email"
              keyboardType="email-address"
              placeholder="Email address"
              placeholderTextColor={colors.textMuted}
              value={email}
              onChangeText={setEmail}
              style={styles.input}
              editable={!busy}
            />
            <TextInput
              autoCapitalize="none"
              autoComplete={creatingAccount ? "new-password" : "current-password"}
              secureTextEntry
              placeholder="Password (at least 6 characters)"
              placeholderTextColor={colors.textMuted}
              value={password}
              onChangeText={setPassword}
              style={styles.input}
              editable={!busy}
            />
            {error ? <Text accessibilityRole="alert" style={styles.error}>{error}</Text> : null}
            <Pressable accessibilityRole="button" disabled={busy || !email.trim() || password.length < 6} style={[styles.button, (busy || !email.trim() || password.length < 6) && styles.buttonDisabled]} onPress={() => void submit()}>
              {busy ? <ActivityIndicator color={colors.bgPrimary} /> : <Text style={styles.buttonText}>{creatingAccount ? "Create account" : "Sign in"}</Text>}
            </Pressable>
            <Pressable disabled={busy} onPress={() => { setCreatingAccount((value) => !value); setError(null); }}>
              <Text style={styles.switchText}>{creatingAccount ? "Already registered? Sign in" : "New to Qoin? Create an account"}</Text>
            </Pressable>
            <Text style={styles.footnote}>Email/password authentication is provided by Firebase. Enable Email/Password in Qoin's Firebase Console before deployment.</Text>
          </>
        ) : (
          <>
            <Text style={styles.eyebrow}>RESEARCH PREVIEW</Text>
            <Text style={styles.title}>Explore your market universe.</Text>
            <Text style={styles.copy}>Qoin's Firebase project is not configured in this build. This preview does not collect credentials or pretend to authenticate you.</Text>
            <View style={styles.notice}>
              <Text style={styles.noticeTitle}>Preview limitations</Text>
              <Text style={styles.noticeCopy}>Your basket stays on this device. Account synchronization is unavailable until Qoin's Firebase project and server settings are configured. No live orders can be placed.</Text>
            </View>
            <Pressable accessibilityRole="button" style={styles.button} onPress={() => navigation.replace("Dashboard")}>
              <Text style={styles.buttonText}>Continue in research preview</Text>
            </Pressable>
          </>
        )}
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
  copy: { color: colors.textMuted, fontSize: 13, lineHeight: 20, marginBottom: 14 },
  input: { backgroundColor: colors.bgPrimary, borderWidth: 1, borderColor: colors.border, borderRadius: 9, paddingHorizontal: 13, paddingVertical: 12, color: colors.textPrimary, fontSize: 13, marginTop: 10 },
  notice: { backgroundColor: colors.bgPrimary, borderRadius: 10, padding: 14, marginTop: 20, borderWidth: 1, borderColor: colors.border },
  noticeTitle: { color: colors.textPrimary, fontSize: 12, fontWeight: "800", marginBottom: 5 },
  noticeCopy: { color: colors.textMuted, fontSize: 11, lineHeight: 17 },
  button: { backgroundColor: colors.accentCyan, borderRadius: 10, padding: 14, alignItems: "center", justifyContent: "center", minHeight: 46, marginTop: 22 },
  buttonDisabled: { opacity: 0.5 },
  buttonText: { color: colors.bgPrimary, fontWeight: "800", fontSize: 13 },
  switchText: { color: colors.accentCyan, fontSize: 11, fontWeight: "700", textAlign: "center", marginTop: 18 },
  error: { color: colors.bearish, fontSize: 11, lineHeight: 16, marginTop: 10 },
  footnote: { color: colors.textMuted, fontSize: 9, lineHeight: 14, marginTop: 16 },
});
