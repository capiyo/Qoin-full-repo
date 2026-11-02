const BLOCKED_USERNAME_PATTERNS:RegExp[]=[/^admin$/i,/^administrator$/i,/^superadmin$/i,/^root$/i,/^sysadmin$/i,/^moderator$/i,/^mod$/i,/^staff$/i,/^support$/i,/^helpdesk$/i,/^webmaster$/i,/^master$/i,/^system$/i,/^test$/i,/^user$/i,/^guest$/i,/^anonymous$/i,/^default$/i,/^example$/i,/^null$/i,/^undefined$/i,/^service$/i,/^api$/i,/^bot$/i,/^cron$/i,/^daemon$/i,/^clash$/i,/^fanclash$/i,/^clashfan$/i,/^clashadmin$/i,/^clashmod$/i,/^clashstaff$/i,/admin/i,/root/i,/super/i,/mod/i];
const SEQUENTIAL=/(?:abc|bcd|cde|def|efg|fgh|ghi|hij|ijk|jkl|klm|lmn|mno|nop|opq|pqr|qrs|rst|stu|tuv|uvw|vwx|wxy|xyz)/i;
const KEYBOARD=/(?:qwerty|asdfgh|zxcvbn|qwert|asdf|zxc|poi|lkj|mnb)/i;
export function validateUsername(username:string):string|null{
 if(!username)return "Username is required";const value=username.trim();if(value.length<3)return "Username must be at least 3 characters";if(value.length>20)return "Username must be 20 characters or fewer";if(!/^[a-zA-Z]+$/.test(value))return "Username must contain only letters (a-z)";const lower=value.toLowerCase();if(BLOCKED_USERNAME_PATTERNS.some(p=>p.test(lower)))return "This username is not allowed";if(/^[0-9+\-\s()]{8,}$/.test(value))return "Username cannot be a phone number";if(/^(.)\1{2,}$/.test(value))return "Username cannot contain only repeated characters";if(SEQUENTIAL.test(lower))return "Username cannot contain sequential patterns";if(KEYBOARD.test(lower))return "Username cannot contain keyboard patterns";return null;
}
export function buildE164(raw:string,dialCode:string):string{
 let value=raw.trim().replace(/[\s\-().]/g,"");if(value.startsWith("+"))return value;if(value.startsWith("00"))return `+${value.slice(2)}`;const digits=dialCode.replace("+","");if(value.startsWith(digits)&&value.length>digits.length+4)return `+${value}`;if(value.startsWith("0"))value=value.slice(1);return `${dialCode}${value}`;
}
export function validatePhone(raw:string,dialCode:string):string|null{
 if(!raw||!raw.trim())return "Phone number is required";const cleaned=raw.trim().replace(/[\s\-()]/g,"");if(!/^\+?[0-9]{7,15}$/.test(cleaned))return "Enter a valid phone number";const e164=buildE164(raw,dialCode);if(!/^\+\d{10,15}$/.test(e164))return "Enter a valid international phone number";return null;
}
export function validatePin(pin:string):string|null{return /^\d{4}$/.test(pin)?null:"Enter a 4-digit PIN"}
