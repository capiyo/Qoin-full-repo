import React, { useEffect, useState } from "react";
import { ActivityIndicator, Pressable, Text, TextInput, View, StyleSheet } from "react-native";
import type { User } from "firebase/auth";
import {
  getQoinInvestmentPlan,
  saveQoinInvestmentPlan,
  type QoinInvestmentPlanInput,
} from "@qoin/core";
import { colors } from "../theme";

export function InvestmentPlanPanel({ user }: { user: User | null }) {
  const [expanded, setExpanded] = useState(false);
  const [goal, setGoal] = useState("");
  const [currency, setCurrency] = useState<QoinInvestmentPlanInput["currency"]>("KES");
  const [monthly, setMonthly] = useState("");
  const [target, setTarget] = useState("");
  const [years, setYears] = useState("10");
  const [saved, setSaved] = useState(false);
  const [loading, setLoading] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (!user) {
      setSaved(false);
      setGoal("");
      setMonthly("");
      setTarget("");
      setYears("10");
      return () => { active = false; };
    }
    setSaved(false);
    setGoal("");
    setMonthly("");
    setTarget("");
    setYears("10");
    setMessage(null);
    setLoading(true);
    void user.getIdToken().then(getQoinInvestmentPlan).then((plan) => {
      if (!active || !plan) return;
      setGoal(plan.goal);
      setCurrency(plan.currency);
      setMonthly((plan.monthly_contribution_minor / 100).toString());
      setTarget(plan.target_amount_minor === null ? "" : (plan.target_amount_minor / 100).toString());
      setYears((plan.horizon_months / 12).toString());
      setSaved(true);
    }).catch(() => {
      if (active) setMessage("Saved plan unavailable. Check your connection.");
    }).finally(() => { if (active) setLoading(false); });
    return () => { active = false; };
  }, [user]);

  async function submit() {
    setMessage(null);
    if (!user) {
      setMessage("Sign in to save your plan securely.");
      return;
    }
    const monthlyValue = Number(monthly);
    const targetValue = target.trim() ? Number(target) : null;
    const yearValue = Number(years);
    if (!goal.trim() || !Number.isFinite(monthlyValue) || monthlyValue <= 0 ||
        (targetValue !== null && (!Number.isFinite(targetValue) || targetValue <= 0)) ||
        !Number.isInteger(yearValue) || yearValue < 1 || yearValue > 50) {
      setMessage("Enter a goal, monthly contribution and a 1–50 year horizon.");
      return;
    }
    const plan: QoinInvestmentPlanInput = {
      goal: goal.trim(),
      currency,
      monthly_contribution_minor: Math.round(monthlyValue * 100),
      target_amount_minor: targetValue === null ? null : Math.round(targetValue * 100),
      horizon_months: yearValue * 12,
    };
    setBusy(true);
    try {
      await user.getIdToken().then((token) => saveQoinInvestmentPlan(plan, token));
      setSaved(true);
      setExpanded(false);
      setMessage("Investment intention saved.");
    } catch {
      setMessage("Plan not saved. Check your connection and retry.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <View style={styles.panel}>
      <View style={styles.heading}>
        <View style={styles.headingCopy}>
          <Text style={styles.eyebrow}>YOUR INVESTMENT INTENTION</Text>
          <Text style={styles.title}>{saved ? "Your plan" : "Start with your goal"}</Text>
          {saved ? <Text style={styles.summary}>{goal} · {currency} {Number(monthly || 0).toLocaleString()} / month · {years} years</Text> : <Text style={styles.summary}>Set your own goal and contribution rhythm.</Text>}
        </View>
        <Pressable disabled={loading} onPress={() => setExpanded((value) => !value)}><Text style={styles.toggle}>{expanded ? "Close" : saved ? "Edit" : "Set a goal"}</Text></Pressable>
      </View>
      {expanded ? (
        <View style={styles.form}>
          <Text style={styles.label}>What are you investing towards?</Text>
          <TextInput maxLength={80} value={goal} onChangeText={setGoal} placeholder="Long-term financial security" placeholderTextColor={colors.textMuted} style={styles.input}/>
          <Text style={styles.label}>Currency</Text>
          <View style={styles.currencyRow}>
            {(["KES","USD","EUR","GBP"] as const).map((item) => <Pressable key={item} onPress={() => setCurrency(item)} style={[styles.currencyChip, currency === item && styles.currencySelected]}><Text style={[styles.currencyText, currency === item && styles.currencyTextSelected]}>{item}</Text></Pressable>)}
          </View>
          <Text style={styles.label}>Monthly contribution</Text>
          <TextInput keyboardType="decimal-pad" value={monthly} onChangeText={setMonthly} placeholder="3000" placeholderTextColor={colors.textMuted} style={styles.input}/>
          <Text style={styles.label}>Target amount (optional)</Text>
          <TextInput keyboardType="decimal-pad" value={target} onChangeText={setTarget} placeholder="500000" placeholderTextColor={colors.textMuted} style={styles.input}/>
          <Text style={styles.label}>Time horizon (years)</Text>
          <TextInput keyboardType="number-pad" value={years} onChangeText={setYears} placeholder="10" placeholderTextColor={colors.textMuted} style={styles.input}/>
          <Pressable disabled={busy || loading} onPress={() => void submit()} style={styles.saveButton}>
            {busy ? <ActivityIndicator color={colors.bgPrimary}/> : <Text style={styles.saveText}>Save my plan</Text>}
          </Pressable>
          {!user ? <Text style={styles.note}>Sign in to save across devices.</Text> : null}
        </View>
      ) : null}
      {message ? <Text accessibilityRole="alert" style={styles.message}>{message}</Text> : null}
      <Text style={styles.note}>Your inputs only. No product recommendation, allocation or transaction is triggered.</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  panel:{marginTop:16,padding:13,backgroundColor:colors.bgPrimary,borderRadius:10,borderWidth:1,borderColor:colors.border},
  heading:{flexDirection:"row",alignItems:"center",justifyContent:"space-between",gap:10},
  headingCopy:{flex:1},
  eyebrow:{color:colors.accentCyan,fontSize:8,fontWeight:"800",letterSpacing:1.2,marginBottom:5},
  title:{color:colors.textPrimary,fontSize:13,fontWeight:"800"},
  summary:{color:colors.textMuted,fontSize:9,lineHeight:14,marginTop:4},
  toggle:{color:colors.accentCyan,fontSize:10,fontWeight:"800"},
  form:{marginTop:12},
  label:{color:colors.textMuted,fontSize:9,fontWeight:"700",marginTop:10,marginBottom:5},
  input:{backgroundColor:colors.bgSecondary,borderWidth:1,borderColor:colors.border,borderRadius:7,paddingHorizontal:10,paddingVertical:9,color:colors.textPrimary,fontSize:11},
  currencyRow:{flexDirection:"row",gap:6},
  currencyChip:{borderWidth:1,borderColor:colors.border,borderRadius:14,paddingVertical:6,paddingHorizontal:10},
  currencySelected:{borderColor:colors.accentCyan,backgroundColor:"#393047"},
  currencyText:{color:colors.textMuted,fontSize:9,fontWeight:"700"},
  currencyTextSelected:{color:colors.accentCyan},
  saveButton:{backgroundColor:colors.accentCyan,borderRadius:8,padding:11,alignItems:"center",marginTop:13},
  saveText:{color:colors.bgPrimary,fontSize:10,fontWeight:"800"},
  message:{color:colors.accentCyan,fontSize:9,marginTop:8},
  note:{color:colors.textMuted,fontSize:8,lineHeight:13,marginTop:8},
});
