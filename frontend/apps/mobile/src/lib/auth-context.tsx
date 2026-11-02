import React,{createContext,useCallback,useContext,useEffect,useMemo,useState} from "react";
import * as SecureStore from "expo-secure-store";
import {getCurrentUser,type QoinSession,type QoinUser} from "@qoin/core";
type AuthValue={session:QoinSession|null;ready:boolean;signIn:(s:QoinSession)=>Promise<void>;signOut:()=>Promise<void>};
const Context=createContext<AuthValue|null>(null);
export function AuthProvider({children}:{children:React.ReactNode}){
 const [session,setSession]=useState<QoinSession|null>(null),[ready,setReady]=useState(false);
 useEffect(()=>{let active=true;(async()=>{try{
  const [token,userJson]=await Promise.all([SecureStore.getItemAsync("qoin_session"),SecureStore.getItemAsync("qoin_user")]);
  if(token){let cached:QoinUser|null=null;try{cached=userJson?JSON.parse(userJson) as QoinUser:null}catch{cached=null}
   try{const current=await getCurrentUser(token);if(active){const restored={success:true,token,user:current.user};setSession(restored);await SecureStore.setItemAsync("qoin_user",JSON.stringify(current.user));}}
   catch(e){const message=e instanceof Error?e.message:"";if(message.includes("401")||message.includes("404")){await SecureStore.deleteItemAsync("qoin_session");await SecureStore.deleteItemAsync("qoin_user");}else if(active&&cached){setSession({success:true,token,user:cached});}}
  }
 }catch{/* Keep startup available; the next explicit sign-in can retry. */}finally{if(active)setReady(true)}})();return()=>{active=false}},[]);
 const signIn=useCallback(async(s:QoinSession)=>{await Promise.all([SecureStore.setItemAsync("qoin_session",s.token),SecureStore.setItemAsync("qoin_user",JSON.stringify(s.user))]);setSession(s)},[]);
 const signOut=useCallback(async()=>{await Promise.all([SecureStore.deleteItemAsync("qoin_session"),SecureStore.deleteItemAsync("qoin_user")]);setSession(null)},[]);
 const value=useMemo(()=>({session,ready,signIn,signOut}),[session,ready,signIn,signOut]);
 return <Context.Provider value={value}>{children}</Context.Provider>;
}
export function useQoinAuth(){const value=useContext(Context);if(!value)throw new Error("useQoinAuth must be used inside AuthProvider");return value}
