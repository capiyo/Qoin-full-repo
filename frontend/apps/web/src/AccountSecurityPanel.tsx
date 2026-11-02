import {useState} from "react";
import {setAccountPin} from "@qoin/core";
export function AccountSecurityPanel({token}:{token:string}){
 const [pin,setPin]=useState(""),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 async function save(){setBusy(true);setMessage("");try{await setAccountPin(token,pin);setPin("");setMessage("PIN saved. You can use it as a sign-in fallback.")}catch(e){setMessage(e instanceof Error?e.message:"Could not save PIN.")}finally{setBusy(false)}}
 return <section className="panel security-panel"><p className="eyebrow">ACCOUNT SECURITY</p><h2>Sign-in PIN</h2><p className="hint">Set a four-digit fallback PIN after phone verification. The backend stores a bcrypt hash, never the PIN itself.</p><div className="security-row"><input aria-label="Four-digit PIN" type="password" inputMode="numeric" maxLength={4} value={pin} onChange={e=>setPin(e.target.value.replace(/\D/g,"").slice(0,4))} placeholder="••••"/><button className="primary" disabled={busy||pin.length!==4} onClick={save}>{busy?"Saving…":"Save PIN"}</button></div>{message&&<p className="hint">{message}</p>}</section>;
}
