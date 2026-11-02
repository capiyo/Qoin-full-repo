import React,{createContext,useCallback,useContext,useEffect,useMemo,useState} from "react";
import * as SecureStore from "expo-secure-store";
import {getCurrentUser,type QoinSession} from "@qoin/core";
type AuthValue={session:QoinSession|null;ready:boolean;signIn:(s:QoinSession)=>Promise<void>;signOut:()=>Promise<void>};
const Context=createContext<AuthValue|null>(null);
export function AuthProvider({children}:{children:React.ReactNode}){
 const [session,setSession]=useState<QoinSession|null>(null),[ready,setReady]=useState(false);
 useEffect(()=>{let active=true;(async()=>{try{const token=await SecureStore.getItemAsync("qoin_session");if(token){const result=await getCurrentUser(token);if(active)setSession({success:true,token,user:result.user});}}catch{await SecureStore.deleteItemAsync("qoin_session");}finally{if(active)setReady(true)}})();return()=>{active=false}},[]);
 const signIn=useCallback(async(s:QoinSession)=>{await SecureStore.setItemAsync("qoin_session",s.token);setSession(s)},[]);
 const signOut=useCallback(async()=>{await SecureStore.deleteItemAsync("qoin_session");setSession(null)},[]);
 const value=useMemo(()=>({session,ready,signIn,signOut}),[session,ready,signIn,signOut]);
 return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function useQoinAuth(){const value=useContext(Context);if(!value)throw new Error("useQoinAuth must be used inside AuthProvider");return value}
