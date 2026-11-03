import React, { useEffect, useMemo, useState } from "react";
import AsyncStorage from "@react-native-async-storage/async-storage";
import { StatusBar } from "expo-status-bar";
import { NavigationContainer } from "@react-navigation/native";
import { createNativeStackNavigator } from "@react-navigation/native-stack";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { View, Pressable, Text, StyleSheet, ScrollView, Modal } from "react-native";
import { SafeAreaProvider, SafeAreaView } from "react-native-safe-area-context";
import { WATCHLIST } from "@qoin/core";
import { colors } from "./src/theme";
import { LoginScreen } from "./src/screens/LoginScreen";
import { useMarketFeed } from "./src/useMarketFeed";

type RootStackParamList = { Login: undefined; Dashboard: undefined };
type Tab = "Archive" | "Feed";
const Stack = createNativeStackNavigator<RootStackParamList>();
const queryClient = new QueryClient();
const PORTFOLIO_STORAGE_KEY = "qoin.portfolio.v1";
const names: Record<string, string> = { "BTC/USD": "Bitcoin", "ETH/USD": "Ethereum", "SOL/USD": "Solana", "AVAX/USD": "Avalanche" };

function MainTabs() {
  const { quotes } = useMarketFeed();
  const [tab, setTab] = useState<Tab>("Archive");
  const [basket, setBasket] = useState<string[]>([]);
  const [basketHydrated, setBasketHydrated] = useState(false);
  const [portfolioOpen, setPortfolioOpen] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [filter, setFilter] = useState("All");
  const assets = useMemo(() => WATCHLIST.map((symbol) => ({ symbol, name: names[symbol] ?? symbol, quote: quotes[symbol] })), [quotes]);
  const visible = filter === "All" || filter === "Crypto" ? assets : [];

  useEffect(() => {
    let active = true;
    async function restoreBasket() {
      try {
        const stored = await AsyncStorage.getItem(PORTFOLIO_STORAGE_KEY);
        if (!stored) return;
        const parsed: unknown = JSON.parse(stored);
        if (!Array.isArray(parsed)) return;
        const allowed = new Set<string>(WATCHLIST);
        const restored = [...new Set(parsed.filter((item): item is string => typeof item === "string" && allowed.has(item)))];
        if (active) setBasket(restored);
      } catch {
        // A corrupt or inaccessible local snapshot must not block the app.
      } finally {
        if (active) setBasketHydrated(true);
      }
    }
    void restoreBasket();
    return () => { active = false; };
  }, []);

  useEffect(() => {
    if (!basketHydrated) return;
    AsyncStorage.setItem(PORTFOLIO_STORAGE_KEY, JSON.stringify(basket)).catch(() => {
      // Device storage can be unavailable; portfolio remains usable in memory.
    });
  }, [basket, basketHydrated]);

  function toggle(symbol: string) {
    setBasket((items) => items.includes(symbol) ? items.filter((item) => item !== symbol) : [...items, symbol]);
  }

  return (
    <SafeAreaView style={s.screen} edges={["top"]}>
      <StatusBar style="light" backgroundColor={colors.bgPrimary} />
      <View style={s.header}>
        <View style={s.brandRow}><View style={s.brandMark}><Text style={s.brandMarkText}>Q</Text></View><Text style={s.brand}>QOIN</Text></View>
        <Pressable style={s.portfolioButton} onPress={() => setPortfolioOpen(true)}><Text style={s.portfolioIcon}>◈</Text><Text style={s.portfolioLabel}>Portfolio</Text><Text style={s.count}>{basket.length}</Text></Pressable>
      </View>

      {selected ? (
        <ScrollView contentContainerStyle={s.content}>
          <Pressable onPress={() => setSelected(null)}><Text style={s.back}>‹ Back to Archive</Text></Pressable>
          <Text style={s.eyebrow}>INDIVIDUAL VECTOR SPACE</Text>
          <Text style={s.title}>{names[selected] ?? selected}</Text>
          <Text style={s.muted}>{selected} · Crypto instrument</Text>
          <View style={s.card}>
            <Text style={s.eyebrow}>FACTOR SPACE</Text><Text style={s.cardTitle}>Instrument vector</Text>
            <View style={s.vector}><View style={s.axisX}/><View style={s.axisY}/><View style={s.axisZ}/><View style={s.origin}/><Text style={s.vectorHint}>Verified factor data will determine vector position</Text></View>
            <Text style={s.disclaimer}>Coordinate-space scaffold only. No live or simulated vector is shown. The complete factor vector remains canonical.</Text>
            {["Return / momentum","Volatility","Market beta","Liquidity / depth","Order-flow imbalance","Funding / basis","Open-interest change"].map((label) => <View key={label} style={s.factor}><Text style={s.muted}>{label}</Text><Text style={s.factorValue}>—</Text></View>)}
            <Pressable style={s.primary} onPress={() => toggle(selected)}><Text style={s.primaryText}>{basket.includes(selected) ? "Remove from portfolio" : "+ Add to portfolio"}</Text></Pressable>
          </View>
        </ScrollView>
      ) : (
        <>
          <View style={s.intro}><Text style={s.eyebrow}>YOUR MARKET UNIVERSE</Text><Text style={s.title}>Discover. Understand.</Text><Text style={s.introSub}>Compose your portfolio.</Text></View>
          <View style={s.tabs}>
            {(["Archive","Feed"] as Tab[]).map((item) => <Pressable key={item} onPress={() => setTab(item)} style={[s.tab, tab === item && s.tabActive]}><Text style={[s.tabText, tab === item && s.tabTextActive]}>{item === "Archive" ? "▤  " : "◉  "}{item}</Text></Pressable>)}
          </View>
          <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={s.list}>
            {tab === "Archive" ? (
              <>
                <View style={s.sectionHead}><View><Text style={s.eyebrow}>INSTRUMENT LIBRARY</Text><Text style={s.sectionTitle}>Archive</Text></View><Text style={s.muted}>{assets.length} instruments</Text></View>
                <ScrollView horizontal showsHorizontalScrollIndicator={false} style={s.filters}>{["All","Forex","Crypto","Stocks","Options"].map((item) => <Pressable key={item} onPress={() => setFilter(item)} style={[s.chip, filter === item && s.chipActive]}><Text style={[s.chipText, filter === item && s.chipTextActive]}>{item}</Text></Pressable>)}</ScrollView>
                {visible.length ? visible.map((asset) => <Pressable key={asset.symbol} style={s.assetRow} onPress={() => setSelected(asset.symbol)}><View style={s.assetIcon}><Text style={s.assetIconText}>{asset.symbol[0]}</Text></View><View style={s.assetInfo}><Text style={s.assetName}>{asset.name}</Text><Text style={s.muted}>{asset.symbol} · Crypto</Text></View><View style={s.quote}><Text style={s.assetName}>{"$"}{asset.quote.price.toLocaleString(undefined,{maximumFractionDigits:2})}</Text><Text style={[s.change,{color:asset.quote.changePct24h>=0?colors.bullish:colors.bearish}]}>{asset.quote.changePct24h>=0?"+":""}{asset.quote.changePct24h.toFixed(2)}%</Text></View><Text style={s.chevron}>›</Text></Pressable>) : <View style={s.empty}><Text style={s.assetName}>No instruments in this category yet</Text><Text style={s.muted}>The connected starter universe currently contains crypto instruments.</Text></View>}
                <Text style={s.disclaimer}>Illustrative data from the repository's mock feed, not exchange-sourced live pricing.</Text>
              </>
            ) : (
              <>
                <View style={s.sectionHead}><View><Text style={s.eyebrow}>MARKET INTELLIGENCE</Text><Text style={s.sectionTitle}>Feed</Text><Text style={s.muted}>Discover instruments for your portfolio.</Text></View></View>
                <View style={s.feedCard}>
                  <View style={s.feedMeta}><View style={s.feedAvatar}><Text style={s.feedAvatarText}>Q</Text></View><View style={s.feedMetaText}><Text style={s.assetName}>Qoin Intelligence</Text><Text style={s.muted}>Instrument discovery</Text></View><Text style={s.feedTag}>DISCOVERY</Text></View>
                  <Text style={s.cardTitle}>Build your market universe</Text><Text style={s.feedCopy}>Inspect available instruments and add assets that match your investment intent.</Text>
                  {assets.map((asset) => <View key={asset.symbol} style={s.feedAsset}><Pressable style={s.assetInfo} onPress={() => setSelected(asset.symbol)}><Text style={s.assetName}>{asset.name}</Text><Text style={s.muted}>{asset.symbol}</Text></Pressable><Pressable onPress={() => toggle(asset.symbol)} style={s.addButton}><Text style={s.addText}>{basket.includes(asset.symbol) ? "In basket ✓" : "+ Add"}</Text></Pressable></View>)}
                  <Text style={s.disclaimer}>Starter universe · Crypto</Text>
                </View>
                <View style={s.feedCard}><View style={s.feedMeta}><View style={s.feedAvatar}><Text style={s.feedAvatarText}>◎</Text></View><View style={s.feedMetaText}><Text style={s.assetName}>News and community</Text><Text style={s.muted}>Financial discovery stream</Text></View><Text style={s.pending}>CONNECTING</Text></View><Text style={s.cardTitle}>Your intelligence feed</Text><Text style={s.feedCopy}>Verified news, research and community discussions will appear when those services are integrated. No sample headlines are presented as real news.</Text></View>
              </>
            )}
          </ScrollView>
          <View style={s.floatingTabs}>{(["Archive","Feed"] as Tab[]).map((item) => <Pressable key={item} onPress={() => setTab(item)} style={[s.floatTab, tab === item && s.floatTabActive]}><Text style={[s.floatText, tab === item && s.floatTextActive]}>{item.toLowerCase()}</Text></Pressable>)}</View>
        </>
      )}

      <Modal visible={portfolioOpen} transparent animationType="slide" onRequestClose={() => setPortfolioOpen(false)}>
        <Pressable style={s.backdrop} onPress={() => setPortfolioOpen(false)}>
          <Pressable style={s.sheet} onPress={() => undefined}>
            <View style={s.handle}/>
            <View style={s.sheetHead}><View><Text style={s.eyebrow}>YOUR ASSET BASKET</Text><Text style={s.sectionTitle}>My portfolio</Text></View><Pressable onPress={() => setPortfolioOpen(false)}><Text style={s.close}>×</Text></Pressable></View>
            <Text style={s.muted}>Choose the instruments you want Qoin to evaluate. Your basket is saved on this device; account synchronization will follow authenticated portfolio storage. Adding an asset does not place an order.</Text>
            {basket.length ? basket.map((symbol) => <View key={symbol} style={s.drawerRow}><View style={s.assetInfo}><Text style={s.assetName}>{names[symbol] ?? symbol}</Text><Text style={s.muted}>{symbol}</Text></View><Pressable onPress={() => toggle(symbol)}><Text style={s.remove}>Remove</Text></Pressable></View>) : <View style={s.empty}><Text style={s.assetName}>Your basket is waiting</Text><Text style={s.muted}>Explore Archive or Feed and add instruments to begin.</Text></View>}
            <View style={s.summary}><View style={s.summaryRow}><Text style={s.muted}>Selected instruments</Text><Text style={s.assetName}>{basket.length}</Text></View><View style={s.summaryRow}><Text style={s.muted}>Management</Text><Text style={s.assetName}>Algorithm-managed</Text></View><View style={s.summaryRow}><Text style={s.muted}>Execution</Text><Text style={s.assetName}>Not enabled</Text></View></View>
            <Text style={s.disclaimer}>Automated management requires verified market data, risk controls and explicit authorization.</Text>
          </Pressable>
        </Pressable>
      </Modal>
    </SafeAreaView>
  );
}

export default function App() {
  return <QueryClientProvider client={queryClient}><SafeAreaProvider><NavigationContainer><Stack.Navigator screenOptions={{headerShown:false}}><Stack.Screen name="Login" component={LoginScreen}/><Stack.Screen name="Dashboard" component={MainTabs}/></Stack.Navigator></NavigationContainer></SafeAreaProvider></QueryClientProvider>;
}

const s = StyleSheet.create({
  screen:{flex:1,backgroundColor:colors.bgPrimary},header:{height:54,paddingHorizontal:16,flexDirection:"row",alignItems:"center",justifyContent:"space-between",backgroundColor:colors.bgSecondary},brandRow:{flexDirection:"row",alignItems:"center",gap:9},brandMark:{width:29,height:29,borderRadius:16,backgroundColor:colors.accentCyan,alignItems:"center",justifyContent:"center"},brandMarkText:{color:colors.bgPrimary,fontWeight:"900",fontSize:16},brand:{color:colors.white,fontSize:14,fontWeight:"900",letterSpacing:2.5},portfolioButton:{flexDirection:"row",alignItems:"center",gap:7,backgroundColor:"#393047",borderRadius:20,paddingVertical:7,paddingHorizontal:11},portfolioIcon:{color:colors.accentCyan,fontSize:18},portfolioLabel:{color:colors.textPrimary,fontSize:11,fontWeight:"700"},count:{color:colors.bgPrimary,backgroundColor:colors.accentCyan,borderRadius:10,overflow:"hidden",fontSize:10,fontWeight:"800",paddingHorizontal:6,paddingVertical:3},intro:{paddingHorizontal:16,paddingTop:24,paddingBottom:16},eyebrow:{color:colors.accentCyan,fontSize:9,fontWeight:"800",letterSpacing:1.5,marginBottom:6},title:{color:colors.white,fontSize:26,fontWeight:"800",letterSpacing:-.7},introSub:{color:colors.textMuted,fontSize:15,marginTop:3},tabs:{flexDirection:"row",gap:20,borderBottomWidth:1,borderBottomColor:colors.border,marginHorizontal:16},tab:{paddingHorizontal:3,paddingBottom:11},tabActive:{borderBottomWidth:2,borderBottomColor:colors.accentCyan},tabText:{color:colors.textMuted,fontSize:12,fontWeight:"700"},tabTextActive:{color:colors.accentCyan},list:{paddingHorizontal:16,paddingTop:18,paddingBottom:100},sectionHead:{flexDirection:"row",justifyContent:"space-between",alignItems:"flex-end",marginBottom:14},sectionTitle:{color:colors.white,fontSize:20,fontWeight:"800"},muted:{color:colors.textMuted,fontSize:10,lineHeight:16},filters:{flexGrow:0,marginBottom:10},chip:{borderWidth:1,borderColor:colors.border,backgroundColor:colors.bgSecondary,borderRadius:18,paddingHorizontal:13,paddingVertical:7,marginRight:7},chipActive:{borderColor:colors.accentCyan,backgroundColor:"#393047"},chipText:{color:colors.textMuted,fontSize:10,fontWeight:"600"},chipTextActive:{color:colors.accentCyan},assetRow:{flexDirection:"row",alignItems:"center",gap:10,paddingVertical:12,borderBottomWidth:1,borderBottomColor:colors.border},assetIcon:{width:34,height:34,borderRadius:18,backgroundColor:colors.bgSecondary,alignItems:"center",justifyContent:"center"},assetIconText:{color:colors.accentCyan,fontWeight:"800"},assetInfo:{flex:1,gap:4},assetName:{color:colors.textPrimary,fontSize:12,fontWeight:"700"},quote:{alignItems:"flex-end",gap:4},change:{fontSize:10,fontWeight:"700"},chevron:{color:colors.textMuted,fontSize:21},empty:{alignItems:"center",padding:24,backgroundColor:colors.bgSecondary,borderRadius:9,marginVertical:10,gap:7},disclaimer:{color:colors.textMuted,fontSize:9,lineHeight:15,marginTop:12},feedCard:{backgroundColor:colors.bgSecondary,padding:14,marginBottom:10,borderBottomWidth:1,borderBottomColor:colors.border},feedMeta:{flexDirection:"row",alignItems:"center",gap:9},feedAvatar:{width:29,height:29,borderRadius:16,backgroundColor:"#393047",alignItems:"center",justifyContent:"center"},feedAvatarText:{color:colors.accentCyan,fontWeight:"800"},feedMetaText:{flex:1,gap:3},feedTag:{color:colors.accentCyan,fontSize:8,fontWeight:"800"},pending:{color:colors.warning,fontSize:8,fontWeight:"800"},cardTitle:{color:colors.white,fontSize:14,fontWeight:"800",marginTop:15,marginBottom:7},feedCopy:{color:colors.textMuted,fontSize:10,lineHeight:16,marginBottom:12},feedAsset:{flexDirection:"row",alignItems:"center",paddingVertical:10,borderTopWidth:1,borderTopColor:colors.border},addButton:{borderWidth:1,borderColor:colors.accentCyan,borderRadius:15,paddingVertical:6,paddingHorizontal:10,backgroundColor:"#393047"},addText:{color:colors.accentCyan,fontSize:9,fontWeight:"800"},floatingTabs:{position:"absolute",bottom:14,alignSelf:"center",flexDirection:"row",gap:4,padding:4,borderRadius:24,backgroundColor:"#302a3d",elevation:7},floatTab:{paddingHorizontal:18,paddingVertical:10,borderRadius:20},floatTabActive:{backgroundColor:colors.accentCyan},floatText:{color:colors.textMuted,fontSize:10,fontWeight:"800"},floatTextActive:{color:colors.bgPrimary},content:{padding:16,paddingBottom:30},back:{color:colors.textMuted,fontSize:11,marginBottom:20},card:{backgroundColor:colors.bgSecondary,borderRadius:10,borderWidth:1,borderColor:colors.border,padding:14,marginTop:16},vector:{height:220,backgroundColor:colors.bgPrimary,borderRadius:8,marginTop:14,marginBottom:10,justifyContent:"center",alignItems:"center",overflow:"hidden"},axisX:{position:"absolute",left:20,right:20,top:"55%",height:1,backgroundColor:"#665d77"},axisY:{position:"absolute",top:16,bottom:20,left:"50%",width:1,backgroundColor:"#665d77"},axisZ:{position:"absolute",width:"80%",height:1,backgroundColor:"#665d77",transform:[{rotate:"-28deg"}]},origin:{width:7,height:7,borderRadius:4,backgroundColor:colors.accentCyan},vectorHint:{color:colors.textMuted,fontSize:9,textAlign:"center",marginTop:26,paddingHorizontal:16},factor:{flexDirection:"row",justifyContent:"space-between",paddingVertical:10,borderBottomWidth:1,borderBottomColor:colors.border},factorValue:{color:colors.textPrimary,fontSize:10,fontWeight:"700"},primary:{marginTop:15,borderRadius:8,backgroundColor:colors.accentCyan,paddingVertical:12,alignItems:"center"},primaryText:{color:colors.bgPrimary,fontSize:11,fontWeight:"800"},backdrop:{flex:1,justifyContent:"flex-end",backgroundColor:"#0009"},sheet:{backgroundColor:colors.bgSecondary,borderTopLeftRadius:18,borderTopRightRadius:18,padding:20,paddingBottom:30,maxHeight:"82%"},handle:{width:38,height:4,borderRadius:3,backgroundColor:colors.textMuted,alignSelf:"center",marginBottom:18},sheetHead:{flexDirection:"row",justifyContent:"space-between",alignItems:"center",marginBottom:10},close:{color:colors.textMuted,fontSize:26},drawerRow:{flexDirection:"row",alignItems:"center",paddingVertical:12,borderBottomWidth:1,borderBottomColor:colors.border},remove:{color:colors.bearish,fontSize:10,fontWeight:"700"},summary:{marginTop:17,backgroundColor:colors.bgPrimary,borderRadius:9,padding:13,gap:12},summaryRow:{flexDirection:"row",justifyContent:"space-between"}
});