//! Shared process state.

use std::sync::Arc;

use dashmap::DashMap;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::config::Config;
use crate::rate_limit::SlidingWindow;
use crate::sim_hub::SimHub;

#[derive(Debug, Clone)]
pub struct KickSignal {
    pub reason: String,
}

struct LiveConn {
    conn_id: Uuid,
    tx: broadcast::Sender<KickSignal>,
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Arc<Config>,
    pub auth_limiter: Arc<SlidingWindow>,
    pub ws_limiter: Arc<SlidingWindow>,
    pub sim: SimHub,
    live_kicks: Arc<DashMap<Uuid, LiveConn>>,
}

impl AppState {
    pub fn new(pool: PgPool, config: Config, sim: SimHub) -> Self {
        Self {
            pool,
            config: Arc::new(config),
            auth_limiter: Arc::new(crate::rate_limit::auth_ip_limiter()),
            ws_limiter: Arc::new(crate::rate_limit::ws_connect_limiter()),
            sim,
            live_kicks: Arc::new(DashMap::new()),
        }
    }

    pub fn claim_character_connection(
        &self,
        character_id: Uuid,
    ) -> (Uuid, broadcast::Receiver<KickSignal>) {
        let conn_id = Uuid::now_v7();
        let (tx, rx) = broadcast::channel(4);
        if let Some((_, old)) = self.live_kicks.remove(&character_id) {
            let _ = old.tx.send(KickSignal {
                reason: "replaced_by_new_connection".into(),
            });
        }
        self.live_kicks
            .insert(character_id, LiveConn { conn_id, tx });
        (conn_id, rx)
    }

    pub fn release_character_connection(&self, character_id: Uuid, conn_id: Uuid) {
        self.live_kicks
            .remove_if(&character_id, |_, live| live.conn_id == conn_id);
    }
}
