#![allow(unused_imports)]
use super::*;
use crate::handlers::*;
use crate::models::*;

pub(crate) async fn run(){
    tracing_subscriber::fmt().with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_|"info".into())).init();
    let mut signal_config = EngineConfig::default();
    if let Ok(value) = env::var("QOIN_SIGNAL_THRESHOLD") { signal_config.signal_threshold = value.parse().expect("QOIN_SIGNAL_THRESHOLD must be numeric"); }
    if let Ok(value) = env::var("QOIN_INPUT_CLIP") { signal_config.input_clip = value.parse().expect("QOIN_INPUT_CLIP must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_SPREAD_BPS") { signal_config.max_spread_bps = value.parse().expect("QOIN_MAX_SPREAD_BPS must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_SPREAD_AGE_MS") { signal_config.max_spread_age_ms = value.parse().expect("QOIN_MAX_SPREAD_AGE_MS must be an integer"); }
    if let Ok(value) = env::var("QOIN_TOP_K") { signal_config.top_k = value.parse().expect("QOIN_TOP_K must be an integer"); }
    if let Ok(value) = env::var("QOIN_MIN_ALIGNMENT") { signal_config.min_alignment = value.parse().expect("QOIN_MIN_ALIGNMENT must be numeric"); }
    if let Ok(value) = env::var("QOIN_MAX_ANGLE_RADIANS") { signal_config.max_angle_radians = value.parse().expect("QOIN_MAX_ANGLE_RADIANS must be numeric"); }
    let signal_strategy_version = env::var("QOIN_STRATEGY_VERSION").or_else(|_| env::var("QOIN_CONFIG_VERSION")).unwrap_or_else(|_| "baseline-v2".into());
    decide_signal(ForceObservation { ofi: 0.0, liquidity: 0.0, volatility: 0.0, funding: 0.0,
        open_interest_change: 0.0, liquidation_pressure: 0.0, basis: 0.0, spread_bps: Some(0.0), spread_age_ms: Some(0) },
        &signal_config, false).expect("Qoin signal engine configuration must be valid");

    let uri=env::var("QOIN_MONGODB_URI").unwrap_or_else(|_|"mongodb://127.0.0.1:27017".into());
    let client=Client::with_uri_str(&uri).await.expect("connect to MongoDB");
    let db=client.database(&env::var("QOIN_MONGODB_DATABASE").unwrap_or_else(|_|"qoin".into()));
    db.run_command(doc!{"ping":1},None).await.expect("MongoDB ping");
    let events:Collection<Document>=db.collection("market_events");
    let dataset_manifests:Collection<Document>=db.collection("market_dataset_manifests");
    let market_backfill_jobs:Collection<Document>=db.collection("market_backfill_jobs");
    let set_ledger_records:Collection<Document>=db.collection("set_relation_ledger");
    let relationship_ledger:Collection<Document>=db.collection("relationship_ledger_events");
    let states:Collection<Document>=db.collection("market_states");
    let decisions:Collection<Document>=db.collection("trade_decisions");
    let strategies:Collection<Document>=db.collection("edge_strategy");
    let orders:Collection<Document>=db.collection("live_orders");
    let trading_controls:Collection<Document>=db.collection("trading_controls");
    let users:Collection<Document>=db.collection("qoin_users");
    let notifications:Collection<Document>=db.collection("notifications");
    let market_vectors:Collection<Document>=db.collection("market_vectors");
    let fcm_tokens:Collection<Document>=db.collection("fcm_tokens");
    let algorithm_selections:Collection<Document>=db.collection("user_algorithm_selections");
    let operating_cycles:Collection<Document>=db.collection("operating_cycles");
    let paper_sessions:Collection<Document>=db.collection("paper_sessions");
    let paper_session_runs:Collection<Document>=db.collection("paper_session_runs");
    let paper_session_decisions:Collection<Document>=db.collection("paper_session_decisions");
    let paper_session_trades:Collection<Document>=db.collection("paper_session_trades");
    let investment_plans:Collection<Document>=db.collection("user_investment_plans");
    let broker_accounts:Collection<Document>=db.collection("broker_accounts");
    let broker_environment = Arc::new(broker_env::BrokerEnvironment::from_env()
        .expect("QOIN_BROKER_* environment configuration must be valid"));
    algorithm_selections.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"symbol":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("one algorithm selection per user and instrument");
    algorithm_selections.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"updated_at":-1}).build(),None).await
        .expect("algorithm selection owner index");
    notifications.create_index(mongodb::IndexModel::builder().keys(doc!{"user_id":1,"created_at":-1}).build(),None).await.expect("user notification history index");
    market_vectors.create_index(mongodb::IndexModel::builder().keys(doc!{"vector_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("market vector idempotency index");
    market_vectors.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_time_cutoff_ms":-1}).build(),None).await.expect("venue scoped market vector history index");
    notifications.create_index(mongodb::IndexModel::builder().keys(doc!{"user_id":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).partial_filter_expression(doc!{"event_id":{"$type":"string"}}).build()).build(),None)
        .await.expect("per-user notification event idempotency index");
    fcm_tokens.create_index(mongodb::IndexModel::builder().keys(doc!{"fcm_token":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("unique FCM device token index");
    fcm_tokens.create_index(mongodb::IndexModel::builder().keys(doc!{"user_id":1}).build(),None).await.expect("user FCM device lookup index");
    broker_accounts.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"broker_id":1,"account_id":1}).build(),None).await
        .expect("broker account owner index");
    broker_accounts.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"broker_id":1,"account_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("broker subaccount must map to only one Firebase user");
    trading_controls.update_one(doc!{"_id":"execution"},
        doc!{"$setOnInsert":{"enabled":false,
            "updated_at":mongodb::bson::DateTime::now()}},
        UpdateOptions::builder().upsert(true).build()).await.expect("initialize trading kill switch");
    orders.create_index(mongodb::IndexModel::builder().keys(doc!{"client_order_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("durable live order idempotency index");
    orders.create_index(mongodb::IndexModel::builder()
        .keys(doc!{"owner_uid":1,"updated_at":-1}).build(),None).await
        .expect("owner-scoped order history index");
    dataset_manifests.create_index(mongodb::IndexModel::builder().keys(doc!{"dataset_version":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("immutable dataset version index");
    market_backfill_jobs.create_index(mongodb::IndexModel::builder().keys(doc!{"job_id":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("unique market backfill job index");
    market_backfill_jobs.create_index(mongodb::IndexModel::builder().keys(doc!{"status":1,"lease_expires_at_ms":1}).build(),None).await.expect("market backfill lease index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"backfill_job_id":1}).build(),None).await.expect("market backfill job event count index");
    dataset_manifests.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"venue":1,"created_at":-1}).build(),None).await.expect("dataset manifest lookup index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"ts_ms":1,"event_id":1}).build(),None).await.expect("historical replay time index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"ts_ms":1,"event_id":1,"venue":1}).build(),None).await.expect("V2 ledger backfill keyset index");
    set_ledger_records.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"event_time_ms":-1,"record_type":1}).build(),None).await.expect("durable V2 ledger symbol/time index");
    set_ledger_records.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"record_type":1,"relation_id":1}).build(),None).await.expect("durable V2 relation lookup index");
    relationship_ledger.create_index(mongodb::IndexModel::builder().keys(doc!{"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await
        .expect("V2 relationship ledger event idempotency index");
    relationship_ledger.create_index(mongodb::IndexModel::builder().keys(doc!{"recorded_at_ms":-1,"event_id":1}).build(),None).await
        .expect("V2 relationship ledger chronological lookup index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"event_id":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("event deduplication index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"received_at_ms":-1})
        .build(),None).await.expect("provider live-ingestion freshness index");
    events.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"ts_ms":-1}).build(),None).await.expect("venue-scoped multi-horizon query index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("state lookup index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"stored_at":-1}).build(),None).await.expect("latest state recovery index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"stored_at":-1}).build(),None).await.expect("decision lookup index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"venue":1,"symbol":1,"strategy_key":1,"stored_at":-1}).build(),None).await.expect("venue scoped vector history index");
    states.create_index(mongodb::IndexModel::builder().keys(doc!{"last_event_venue":1,"symbol":1,"stored_at":-1}).build(),None).await.expect("venue scoped state history index");
    decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"symbol":1,"input_event.event_ts_ms":-1}).build(),None).await.expect("historical source-event vector lookup index");
    strategies.create_index(mongodb::IndexModel::builder().keys(doc!{"strategy_key":1})
        .options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("strategy key index");
    let baseline=EdgeStrategy::baseline();
    let active=match strategies.find_one(doc!{"strategy_key":STRATEGY_KEY},None).await.expect("load edge strategy map"){
        Some(doc)=>match mongodb::bson::from_document::<EdgeStrategy>(doc){Ok(v)=>{v.validate().expect("stored edge strategy must validate");v},
            Err(e)=>panic!("stored edge strategy map is invalid: {e}")},
        None=>{
            let d=mongodb::bson::to_document(&baseline).expect("serialize baseline strategy map");
            strategies.insert_one(d,None).await.expect("seed baseline edge strategy map");
            baseline
        }
    };
    tracing::info!(strategy_key=%active.strategy_key,version=%active.version,"loaded legacy edge-map compatibility settings; active decisions use qoin-vector-engine");
    // Rebuild rolling state from the immutable event log on every process start.
    // Snapshot-only recovery can preserve a stale snapshot when a crash occurs
    // after event insertion but before snapshot persistence. Replaying the raw
    // log closes that specific recovery gap and uses deterministic event-time
    // ordering across providers. Malformed legacy rows are skipped and surfaced.
    let replay_options = FindOptions::builder()
        .sort(doc!{"symbol":1,"ts_ms":1,"event_id":1,"venue":1})
        .build();
    let mut event_cursor = events.find(doc!{"historical_only":{"$ne":true}}, Some(replay_options)).await
        .expect("open immutable market event log for recovery");
    let mut restored_memory: HashMap<String, SymbolState> = HashMap::new();
    let mut replayed_events: u64 = 0;
    let mut skipped_events: u64 = 0;
    while let Some(document) = event_cursor.try_next().await
        .expect("read immutable market event log during recovery") {
        match mongodb::bson::from_document::<MarketEvent>(document) {
            Ok(event) if validate_event(&event).is_ok() => {
                let state = restored_memory.entry(venue_state_key(&event.venue,&event.symbol)).or_default();
                apply(state, &event);
                replayed_events += 1;
            }
            Ok(event) => {
                skipped_events += 1;
                tracing::warn!(symbol=%event.symbol,event_id=%event.event_id,
                    "skipping invalid persisted market event during recovery");
            }
            Err(error) => {
                skipped_events += 1;
                tracing::warn!(error=%error,
                    "skipping malformed persisted market event during recovery");
            }
        }
    }
    tracing::info!(restored_symbols=restored_memory.len(),replayed_events,skipped_events,
        "rebuilt market snapshots from immutable MongoDB event log");
    operating_cycles.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("per-user operating cycle index");
    paper_sessions.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"session_key":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("per-user paper session state index");
    paper_session_runs.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("per-user paper session archive index");
    paper_session_decisions.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"venue":1,"symbol":1,"event_id":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("per-user paper Mirror event index");
    paper_session_trades.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1,"cycle_id":1,"venue":1,"symbol":1,"exit_ts_ms":1}).build(),None).await.expect("per-user paper trade history index");
    investment_plans.create_index(mongodb::IndexModel::builder().keys(doc!{"owner_uid":1}).options(IndexOptions::builder().unique(true).build()).build(),None).await.expect("per-user investment plan index");
    let firebase_verifier = env::var("QOIN_FIREBASE_PROJECT_ID").ok()
        .filter(|project_id| !project_id.trim().is_empty())
        .map(firebase_identity::FirebaseIdentityVerifier::new)
        .transpose()
        .expect("configure a valid Firebase project ID for authenticated trading routes");
    let (stream, _) = broadcast::channel(4096);
    let state=AppState{
        stream, database: db.clone(), events,dataset_manifests,market_backfill_jobs,set_ledger_records,relationship_ledger,states,decisions,strategies,orders,trading_controls,
        users, notifications, fcm_tokens, market_vectors, algorithm_selections, broker_accounts,
        broker_registry: broker_contract::BrokerRegistry::default(),
        broker_environment,
        firebase_verifier,
        memory:Arc::new(RwLock::new(restored_memory)),
        symbol_ingest_locks:Arc::new((0..64).map(|_| tokio::sync::Mutex::new(())).collect()),
        active_strategy:Arc::new(RwLock::new(active)),
        signal_config,
        signal_strategy_version,
        recovery_replayed_events: replayed_events,
        recovery_skipped_events: skipped_events,
        operating_cycles, paper_sessions, paper_session_runs, paper_session_decisions, paper_session_trades, investment_plans,
        active_cycles: Arc::new(Mutex::new(HashMap::new())),
        active_paper_sessions: Arc::new(Mutex::new(HashMap::new())),
    };
    let app=Router::new().route("/health",get(health)).route("/internal/brokers",get(broker_catalogue::get_brokers)).route("/v1/signal",post(evaluate_signal))
        .route("/internal/users/me",get(get_qoin_me))
        .route("/internal/me/algorithm-selections",get(get_algorithm_selections).post(put_algorithm_selection))
        .route("/internal/me/algorithm-decision",get(get_algorithm_decision))
        .route("/internal/broker-accounts",get(get_broker_accounts))
        .route("/internal/trading/readiness",get(broker_env_api::get_broker_readiness))
        .route("/internal/brokers/configuration",get(broker_env_api::get_broker_configuration))
        .route("/internal/notifications",get(notifications::list))
        .route("/internal/market-vector",get(market_vector_api::get_latest))
        .route("/internal/market-vector/history",get(market_vector_api::get_history))
        .route("/internal/notifications/unread-summary",get(notifications::unread_summary))
        .route("/internal/notifications/register-token",post(notifications::register_token))
        .route("/internal/notifications/record",post(notifications::record_client_event))
        .route("/internal/notifications/mark-read",post(notifications::mark_read))
        .route("/internal/notifications/preferences",get(notifications::get_preferences).post(notifications::update_preferences))
        .route("/internal/notifications/send",post(notifications::send_internal))
        .route("/internal/admin/broker-accounts",post(broker_env_api::provision_broker_account))
        .route("/internal/orders",post(submit_live_order))
        .route("/internal/orders/:client_order_id/reconcile",get(reconcile_live_order))
        .route("/internal/trading/kill-switch",post(set_trading_enabled))
        .route("/internal/stream",get(stream_ws)).route("/internal/events",get(get_events).post(ingest))
        .route("/internal/state",get(get_state)).route("/internal/vector",get(get_vector))
        .route("/internal/vector/history",get(get_vector_history))
        .route("/internal/set-ledger",get(get_set_ledger))
        .route("/internal/set-ledger/reconstruct",get(reconstruct_set_ledger))
        .route("/internal/set-ledger/backfill",post(backfill_set_ledger))
         .route("/internal/set-ledger/corrections",get(list_set_ledger_corrections).post(propose_set_ledger_correction))
        .route("/internal/set-ledger/integrity",get(audit_set_ledger))
        .route("/internal/v2/relationships",get(v2_relationships::list_relationship_events).post(v2_relationships::append_relationship_event))
        .route("/internal/v2/relationships/discover",post(v2_relationships::discover_relationships))
        .route("/internal/datasets",get(list_dataset_manifests).post(create_dataset_manifest))
        .route("/internal/market-data/providers/health",get(get_market_provider_health))
        .route("/internal/market-data/backfill/binance-usdm",post(backfill_binance_usdm_aggregate_trades))
        .route("/internal/market-data/backfill/:job_id",get(get_market_backfill_job))
        .route("/internal/backtest",get(run_backtest)).route("/internal/instruments",get(get_instruments))
        .route("/internal/news",get(get_news)).route("/internal/calendar",get(get_calendar))
        .route("/internal/strategy",get(get_strategy).put(put_strategy))
        .route("/internal/algorithms",get(algorithm_catalogue::get_algorithms))
        .route("/internal/operating-cycle",get(get_operating_cycle).put(configure_operating_cycle))
        .route("/internal/operating-cycle/start",post(start_operating_cycle))
        .route("/internal/operating-cycle/stop",post(stop_operating_cycle))
        .route("/internal/paper-session",get(get_paper_session))
        .route("/internal/paper-session/history",get(get_paper_session_history))
        .route("/internal/paper-session/runs",get(get_paper_session_runs))
        .route("/internal/investment-plan",get(get_investment_plan).put(put_investment_plan))
        .route("/internal/market-horizons",get(get_market_horizons))
        .route("/internal/multi-horizon",post(evaluate_multi_horizon_inputs))
        .with_state(state).layer(TraceLayer::new_for_http());
    let addr=env::var("QOIN_AXUM_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    // Bind address may be 0.0.0.0 inside a container; provider callbacks must
    // connect through a concrete local interface instead of the wildcard address.
    let internal_addr=env::var("QOIN_AXUM_INTERNAL_ADDR").unwrap_or_else(|_|"127.0.0.1:8081".into());
    let binance_enabled=stream_enabled("QOIN_BINANCE_STREAM_ENABLED", true);
    if binance_enabled {
        let symbols=env::var("QOIN_BINANCE_STREAM_SYMBOLS").unwrap_or_else(|_|"BTCUSDT,ETHUSDT,SOLUSDT,BNBUSDT,XRPUSDT,DOGEUSDT".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(|v|v.to_ascii_uppercase()).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        let open_interest_addr=internal_addr.clone();
        let open_interest_symbols=symbols.clone();
        tokio::spawn(async move{spawn_binance_stream(adapter_addr,symbols).await;});
        tokio::spawn(async move{spawn_binance_open_interest(open_interest_addr,open_interest_symbols).await;});
    } else { tracing::info!("Binance public market stream disabled by configuration"); }
    // Public read-only market data is enabled by default; set the flag to false to opt out.
    let kraken_enabled=stream_enabled("QOIN_KRAKEN_STREAM_ENABLED", true);
    if kraken_enabled {
        let symbols=env::var("QOIN_KRAKEN_STREAM_SYMBOLS").unwrap_or_else(|_|"BTC/USD,ETH/USD,SOL/USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_kraken_stream(adapter_addr,symbols).await; });
    } else {
        tracing::info!("Kraken Spot public market stream disabled by configuration");
    }
    // Public read-only market data is enabled by default; set the flag to false to opt out.
    let coinbase_enabled=stream_enabled("QOIN_COINBASE_STREAM_ENABLED", true);
    if coinbase_enabled {
        let products=env::var("QOIN_COINBASE_STREAM_PRODUCTS").unwrap_or_else(|_|"BTC-USD,ETH-USD,SOL-USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_coinbase_stream(adapter_addr,products).await; });
    } else {
        tracing::info!("Coinbase public market stream disabled by configuration");
    }
    if let Ok(token) = env::var("FINNHUB_API_KEY") {
      if !token.trim().is_empty() {
        let symbols=env::var("QOIN_FINNHUB_STREAM_SYMBOLS").unwrap_or_else(|_|"AAPL,MSFT,NVDA,AMZN,GOOGL,META,TSLA,OANDA:EUR_USD,OANDA:GBP_USD,OANDA:USD_JPY,OANDA:AUD_USD,OANDA:SPX500_USD,OANDA:NAS100_USD,OANDA:US30_USD,OANDA:DE30_EUR,OANDA:XAU_USD,OANDA:XAG_USD,OANDA:WTICO_USD,OANDA:BCO_USD,OANDA:NATGAS_USD".into())
            .split(',').map(str::trim).filter(|v|!v.is_empty()).map(str::to_owned).collect::<Vec<_>>();
        let adapter_addr=internal_addr.clone();
        tokio::spawn(async move { spawn_finnhub_stream(adapter_addr,token,symbols).await; });
      } else { tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is empty"); }
    } else {
        tracing::info!("Finnhub market stream disabled: FINNHUB_API_KEY is not configured");
    }
    let listener=tokio::net::TcpListener::bind(&addr).await.expect("bind Axum listener");
    tracing::info!("Qoin Axum research engine listening on {addr}; live orders remain disconnected");
    axum::serve(listener,app).await.expect("Axum server failed");
}
