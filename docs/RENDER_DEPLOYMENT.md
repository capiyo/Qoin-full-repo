# Qoin deployment on Render

Qoin deploys from the repository's authoritative `master` branch using the root
`render.yaml` Blueprint. The Blueprint provisions three services in Frankfurt:

- `qoin-axum`: private Rust/Axum decision and research service.
- `qoin-gateway`: public C++/Drogon HTTP and WebSocket gateway.
- `qoin-web`: public Vite static web client.

The Rust service is private. Browser and mobile clients must use the gateway;
they must never call Axum's internal routes directly.

## Required setup

1. Create a MongoDB database outside Render (MongoDB Atlas is supported).
   Create a dedicated least-privilege application user and restrict network
   access to the Render deployment as supported by the database provider.
2. Connect the GitHub repository `capiyo/Qoin-full-repo` to Render and create
   a Blueprint from the root `render.yaml`. Select `master` as the deploy
   branch.
3. Before the first successful deploy, set the Blueprint's unsynced values:
   - `QOIN_MONGODB_URI`: TLS-enabled MongoDB connection string for the Qoin
     application user and the `qoin` database.
   - `QOIN_FIREBASE_PROJECT_ID`: Firebase project ID used by the clients.
   - `QOIN_CREDENTIAL_SEAL_PUBLIC_KEY_HEX` and
     `QOIN_CREDENTIAL_SEAL_SECRET_KEY_HEX`: the deployment's generated,
     matching credential-sealing key pair. Keep the secret key private.
   - `QOIN_EXECUTION_TOKEN` and `QOIN_LEDGER_MAINTENANCE_TOKEN`: generate
     independent high-entropy secrets; do not reuse Firebase or database
     credentials.
   - `FINNHUB_API_KEY`: optional market-data key.
   - `VITE_QOIN_API_URL`: the public HTTPS URL of `qoin-gateway`, e.g.
     `https://qoin-gateway.onrender.com`. Use the actual hostname Render
     assigns to the service.
4. Deploy the Blueprint. The web client and gateway are public; Axum remains
   private on the Render private network.

## Runtime and safety

- Deploys automatically from `master`; no feature or deployment branch is
  required.
- The gateway resolves Axum's private hostname and port through Render's `fromService` Blueprint reference, then proxies HTTP and WebSocket traffic over the private network. Its listener binds to Render's injected public-service port (`10000`).
- MongoDB is external to the Blueprint so database persistence is not tied to
  an ephemeral application filesystem.
- `QOIN_TRADING_ENABLED=false` is explicit. This deployment is research and
  paper-trading only; market-data connectivity does not enable live orders.
- Do not put broker credentials, Firebase service-account private keys, MongoDB
  passwords, or sealing secrets in Git, frontend variables, or build arguments.
- Configure production Firebase Auth and Firestore rules independently. The
  browser receives only public Firebase configuration; server-side identity
  verification remains in Axum.

## Verification after deployment

- `https://<gateway-host>/health` must return a successful health response.
- Confirm the gateway can reach Axum and Axum can authenticate to MongoDB.
- Load the web app and verify its API base points to the gateway HTTPS URL.
- Verify authenticated API requests with a real Firebase ID token.
- Verify market-data health and paper-session persistence after a service
  restart.
- Confirm live trading remains disabled.
