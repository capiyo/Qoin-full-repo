import { registerRootComponent } from "expo";
import App from "./App";
import {registerBackgroundNotifications} from "./src/lib/firebase";
registerBackgroundNotifications();

registerRootComponent(App);
