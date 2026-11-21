const qoinDb = db.getSiblingDB(process.env.QOIN_MONGODB_DATABASE || "qoin");
const appUser = process.env.QOIN_MONGODB_APP_USERNAME;
const appPassword = process.env.QOIN_MONGODB_APP_PASSWORD;
if (!appUser || !appPassword) {
  throw new Error("QOIN_MONGODB_APP_USERNAME and QOIN_MONGODB_APP_PASSWORD are required");
}
if (!qoinDb.getUser(appUser)) {
  qoinDb.createUser({
    user: appUser,
    pwd: appPassword,
    roles: [{ role: "readWrite", db: qoinDb.getName() }]
  });
}
