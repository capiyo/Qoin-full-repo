import {Platform,PermissionsAndroid} from "react-native";
import {getAuth,signInWithPhoneNumber} from "@react-native-firebase/auth";
import {getMessaging,requestPermission,getToken} from "@react-native-firebase/messaging";
export const firebaseAuth=getAuth();
export const firebaseMessaging=getMessaging();
export {signInWithPhoneNumber};
export async function requestDevicePushToken():Promise<string|null>{
 if(Platform.OS==="android"&&Number(Platform.Version)>=33){const result=await PermissionsAndroid.request(PermissionsAndroid.PERMISSIONS.POST_NOTIFICATIONS);if(result!==PermissionsAndroid.RESULTS.GRANTED)return null;}
 const status=await requestPermission(firebaseMessaging);
 if(status!==1&&status!==2)return null;
 try{return await getToken(firebaseMessaging)}catch{return null}
}
