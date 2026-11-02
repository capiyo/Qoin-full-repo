import { useEffect,useState } from "react";
import { getNotifications,getNotificationPreferences,getUnreadSummary,markNotificationsRead,updateNotificationPreferences,type QoinNotification,type NotificationPreferences } from "@qoin/core";
export function NotificationsPanel({token,onEnablePush}:{token:string;onEnablePush:()=>void}){
 const [items,setItems]=useState<QoinNotification[]>([]),[count,setCount]=useState(0),[prefs,setPrefs]=useState<NotificationPreferences>({market_alerts:true,security_alerts:true,system_alerts:true}),[error,setError]=useState("");
 async function refresh(){try{const [n,u,p]=await Promise.all([getNotifications(token),getUnreadSummary(token),getNotificationPreferences(token)]);setItems(n.notifications);setCount(u.notifications);setPrefs(p);}catch(e){setError(e instanceof Error?e.message:"Notifications unavailable.");}}
 useEffect(()=>{void refresh();},[token]);
 async function markAll(){try{await markNotificationsRead(token);await refresh();}catch(e){setError(e instanceof Error?e.message:"Could not mark notifications read.");}}
 async function save(next:NotificationPreferences){setPrefs(next);try{await updateNotificationPreferences(token,next);}catch(e){setError(e instanceof Error?e.message:"Could not save preferences.");}}
 return <section className="panel notifications-panel"><div className="panel-heading"><div><p className="eyebrow">ACCOUNT ACTIVITY</p><h2>Notifications <span className="counter">{count} unread</span></h2></div><button className="text-button" onClick={markAll}>Mark all read</button></div>
 <button className="text-button" onClick={onEnablePush}>Enable browser push alerts</button><div className="notification-list">{items.length?items.map(n=><article className={n.is_read?"notification-item":"notification-item unread"} key={n.id}><div><strong>{n.title}</strong><p>{n.body}</p><small>{new Date(n.created_at).toLocaleString()}</small></div></article>):<p className="hint">No notifications yet.</p>}</div>
 <div className="notification-prefs"><strong>Notification preferences</strong>{(["market_alerts","security_alerts","system_alerts"] as const).map(k=><label key={k}><input type="checkbox" checked={prefs[k]} onChange={e=>void save({...prefs,[k]:e.target.checked})}/>{k.replace("_"," ").replace(/\b\w/g,c=>c.toUpperCase())}</label>)}</div>{error&&<p className="error">{error}</p>}
 </section>;
}
