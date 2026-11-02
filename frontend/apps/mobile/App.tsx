import React,{useState} from "react";
import {StatusBar} from "expo-status-bar";
import {QueryClient,QueryClientProvider} from "@tanstack/react-query";
import {View,Pressable,Text,StyleSheet} from "react-native";
import {SafeAreaProvider} from "react-native-safe-area-context";
import {colors} from "./src/theme";
import {LoginScreen} from "./src/screens/LoginScreen";
import {DashboardScreen} from "./src/screens/DashboardScreen";
import {PositionsScreen} from "./src/screens/PositionsScreen";
import {SettingsScreen} from "./src/screens/SettingsScreen";
import {SignalScreen} from "./src/screens/SignalScreen";
import {NotificationsScreen} from "./src/screens/NotificationsScreen";
import {AuthProvider,useQoinAuth} from "./src/lib/auth-context";
type Tab="Dashboard"|"Positions"|"Signal"|"Notifications"|"Settings";
const queryClient=new QueryClient();
function TabBar({current,onChange}:{current:Tab;onChange:(tab:Tab)=>void}){
 const tabs:Tab[]=["Dashboard","Positions","Signal","Notifications","Settings"];
 return <View style={styles.tabBar}>{tabs.map(tab=><Pressable key={tab} onPress={()=>onChange(tab)} style={styles.tabItem}><Text style={{color:current===tab?colors.accentCyan:colors.textMuted,fontWeight:"600",fontSize:11}}>{tab}</Text></Pressable>)}</View>
}
function MainTabs(){
 const [tab,setTab]=useState<Tab>("Dashboard");const {session,signOut}=useQoinAuth();
 return <View style={styles.main}><View style={styles.accountBar}><Text style={styles.brand}>QOIN</Text><Text style={styles.username}>{session?.user.username}</Text><Pressable onPress={()=>void signOut()}><Text style={styles.signOut}>Sign out</Text></Pressable></View><View style={styles.content}>
 {tab==="Dashboard"&&<DashboardScreen/>}{tab==="Positions"&&<PositionsScreen/>}{tab==="Signal"&&<SignalScreen/>}{tab==="Notifications"&&<NotificationsScreen/>}{tab==="Settings"&&<SettingsScreen/>}
 </View><TabBar current={tab} onChange={setTab}/></View>
}
function AppNavigator(){const {session,ready}=useQoinAuth();if(!ready)return <View style={styles.loading}><Text style={styles.loadingText}>Restoring secure session…</Text></View>;return session?<MainTabs/>:<LoginScreen/>}
export default function App(){return <AuthProvider><QueryClientProvider client={queryClient}><SafeAreaProvider><StatusBar style="light" backgroundColor={colors.bgPrimary}/><AppNavigator/></SafeAreaProvider></QueryClientProvider></AuthProvider>}
const styles=StyleSheet.create({main:{flex:1,backgroundColor:colors.bgPrimary},accountBar:{height:48,flexDirection:"row",alignItems:"center",paddingHorizontal:14,gap:12,borderBottomWidth:1,borderBottomColor:colors.border},brand:{color:colors.white,fontSize:13,fontWeight:"900",letterSpacing:2},username:{color:colors.textMuted,fontSize:11,flex:1,textAlign:"right"},signOut:{color:colors.accentCyan,fontSize:11,fontWeight:"700"},content:{flex:1},tabBar:{flexDirection:"row",borderTopWidth:1,borderTopColor:colors.border,backgroundColor:colors.bgSecondary,paddingVertical:10},tabItem:{flex:1,alignItems:"center"},loading:{flex:1,backgroundColor:colors.bgPrimary,justifyContent:"center"},loadingText:{color:colors.textMuted,textAlign:"center"}});
