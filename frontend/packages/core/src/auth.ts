import { qoinRequest } from "./qoinApi";
export interface QoinUser{id:string;username:string;phone:string;entitlements:{forex?:boolean;crypto?:boolean;stocks?:boolean;strategies?:string[]}}
export interface QoinSession{success:boolean;token:string;user:QoinUser}
export interface QoinNotification{id:string;notification_type:string;title:string;body:string;data:Record<string,unknown>;is_read:boolean;created_at:string}
export interface NotificationPreferences{market_alerts:boolean;security_alerts:boolean;system_alerts:boolean}
export const loginWithFirebaseToken=(id_token:string)=>qoinRequest<QoinSession>("/api/auth/firebase-login",{method:"POST",body:JSON.stringify({id_token})});
export const registerWithFirebaseToken=(id_token:string,username:string)=>qoinRequest<QoinSession>("/api/auth/register",{method:"POST",body:JSON.stringify({id_token,username})});
export const loginWithPin=(phone:string,pin:string)=>qoinRequest<QoinSession>("/api/auth/pin-login",{method:"POST",body:JSON.stringify({phone,pin})});
export const setAccountPin=(token:string,pin:string)=>qoinRequest<QoinSession>("/api/auth/set-pin",{method:"POST",token,body:JSON.stringify({pin})});
export const getCurrentUser=(token:string)=>qoinRequest<{user:QoinUser}>("/api/auth/me",{token});
export const registerPushToken=(token:string,fcm_token:string,platform:"web"|"android"|"ios")=>qoinRequest("/api/notifications/register-token",{method:"POST",token,body:JSON.stringify({fcm_token,platform})});
export const getNotifications=(token:string)=>qoinRequest<{notifications:QoinNotification[]}>("/api/notifications",{token});
export const getUnreadSummary=(token:string)=>qoinRequest<{notifications:number;comments:number}>("/api/notifications/unread-summary",{token});
export const markNotificationsRead=(token:string,notification_ids?:string[])=>qoinRequest("/api/notifications/mark-read",{method:"PUT",token,body:JSON.stringify({notification_ids})});
export const getNotificationPreferences=(token:string)=>qoinRequest<NotificationPreferences>("/api/notifications/preferences",{token});
export const updateNotificationPreferences=(token:string,preferences:NotificationPreferences)=>qoinRequest("/api/notifications/preferences",{method:"PUT",token,body:JSON.stringify(preferences)});

export function checkQoinUser(phone: string): Promise<{success:boolean;exists:boolean;has_pin:boolean;user?:{id:string;username:string;phone:string}}> {
  return qoinRequest(`/api/auth/check-user/${encodeURIComponent(phone)}`);
}
export async function isQoinUsernameTaken(username:string):Promise<boolean>{
  try{const result=await qoinRequest<{success:boolean;user:{id:string;username:string}|null}>(`/api/auth/user/username/${encodeURIComponent(username)}`);return result.success&&result.user!==null}catch{return false}
}
