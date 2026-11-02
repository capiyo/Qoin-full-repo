import { useRef, useState } from "react";
import { RecaptchaVerifier, signInWithPhoneNumber, type ConfirmationResult } from "firebase/auth";
import { loginWithFirebaseToken, loginWithPin, registerWithFirebaseToken, type QoinSession } from "@qoin/core";
import { getFirebaseAuth } from "./firebase";

export function AuthPanel({ onAuthenticated }: { onAuthenticated: (session: QoinSession) => void }) {
  const [step,setStep]=useState<"phone"|"otp"|"username"|"pin">("phone");
  const [phone,setPhone]=useState("+254");
  const [code,setCode]=useState("");
  const [pin,setPin]=useState("");
  const [username,setUsername]=useState("");
  const [error,setError]=useState("");
  const [busy,setBusy]=useState(false);
  const confirmation=useRef<ConfirmationResult|null>(null);
  const verifier=useRef<RecaptchaVerifier|null>(null);
  const idToken=useRef("");
  async function sendOtp(){
    setBusy(true);setError("");
    try{
      const auth=getFirebaseAuth();
      verifier.current?.clear();
      verifier.current=new RecaptchaVerifier(auth,"qoin-recaptcha",{size:"invisible"});
      confirmation.current=await signInWithPhoneNumber(auth,phone.trim(),verifier.current);
      setStep("otp");
    }catch(e){setError(e instanceof Error?e.message:"Could not send verification code.");}
    finally{setBusy(false);}
  }
  async function verifyOtp(){
    if(!confirmation.current)return;
    setBusy(true);setError("");
    try{
      const result=await confirmation.current.confirm(code.trim());
      idToken.current=await result.user.getIdToken();
      try{const session=await loginWithFirebaseToken(idToken.current);onAuthenticated(session);}
      catch(e){if(e instanceof Error&&e.message.includes("404"))setStep("username");else throw e;}
    }catch(e){setError(e instanceof Error?e.message:"Phone verification failed.");}
    finally{setBusy(false);}
  }
  async function register(){
    setBusy(true);setError("");
    try{onAuthenticated(await registerWithFirebaseToken(idToken.current,username.trim()));}
    catch(e){setError(e instanceof Error?e.message:"Registration failed.");}
    finally{setBusy(false);}
  }
  async function pinLogin(){
    setBusy(true);setError("");
    try{onAuthenticated(await loginWithPin(phone.trim(),pin));}
    catch(e){setError(e instanceof Error?e.message:"PIN login failed.");}
    finally{setBusy(false);}
  }
  return <section className="auth-panel panel">
    <p className="eyebrow">ACCOUNT ACCESS</p><h2>Sign in to Qoin</h2>
    <p className="hint">Verify your phone number. Your account and market permissions are loaded from the Qoin backend.</p>
    {step==="phone"&&<><label className="auth-label">Phone number (international format)<input value={phone} onChange={e=>setPhone(e.target.value)} placeholder="+2547..." /></label><button className="primary" disabled={busy||phone.trim().length<8} onClick={sendOtp}>{busy?"Sending…":"Continue with phone"}</button><button className="text-button" onClick={()=>setStep("pin")}>Use my 4-digit PIN instead</button></>}
    {step==="otp"&&<><label className="auth-label">Verification code<input inputMode="numeric" value={code} onChange={e=>setCode(e.target.value.replace(/\D/g,"").slice(0,6))} placeholder="6-digit code" /></label><button className="primary" disabled={busy||code.length!==6} onClick={verifyOtp}>{busy?"Verifying…":"Verify phone"}</button><button className="text-button" onClick={()=>setStep("phone")}>Change phone number</button></>}
    {step==="username"&&<><label className="auth-label">Choose a username<input value={username} onChange={e=>setUsername(e.target.value.replace(/[^a-zA-Z0-9_]/g,"").slice(0,24))} placeholder="3–24 letters, numbers or _" /></label><button className="primary" disabled={busy||username.length<3} onClick={register}>{busy?"Creating account…":"Create Qoin account"}</button></>}
    {step==="pin"&&<><label className="auth-label">Phone number<input value={phone} onChange={e=>setPhone(e.target.value)} /></label><label className="auth-label">4-digit PIN<input type="password" inputMode="numeric" maxLength={4} value={pin} onChange={e=>setPin(e.target.value.replace(/\D/g,"").slice(0,4))} /></label><button className="primary" disabled={busy||pin.length!==4} onClick={pinLogin}>{busy?"Signing in…":"Sign in with PIN"}</button><button className="text-button" onClick={()=>setStep("phone")}>Use phone verification</button></>}
    {error&&<p className="error" role="alert">{error}</p>}<div id="qoin-recaptcha" />
  </section>;
}
