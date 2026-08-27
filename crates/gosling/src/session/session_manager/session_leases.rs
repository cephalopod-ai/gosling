use super::SessionStorage;
use anyhow::Result;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

const LEASE_TTL: Duration = Duration::from_secs(90);
const LEASE_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) struct SessionTurnLease {
    storage: Arc<SessionStorage>,
    session_id: String,
    lease_id: String,
    heartbeat_cancel: CancellationToken,
    released: bool,
}

impl SessionTurnLease {
    #[cfg(test)]
    pub(crate) async fn release(mut self) -> Result<()> {
        self.heartbeat_cancel.cancel();
        self.storage
            .release_session_turn_lease(&self.session_id, &self.lease_id)
            .await?;
        self.released = true;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn abandon(mut self) {
        self.heartbeat_cancel.cancel();
        self.released = true;
    }
}

impl Drop for SessionTurnLease {
    fn drop(&mut self) {
        self.heartbeat_cancel.cancel();
        if self.released {
            return;
        }
        let storage = Arc::clone(&self.storage);
        let session_id = self.session_id.clone();
        let lease_id = self.lease_id.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = storage
                    .release_session_turn_lease(&session_id, &lease_id)
                    .await;
            });
        }
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

impl SessionStorage {
    pub(super) async fn acquire_session_turn_lease(
        self: Arc<Self>,
        session_id: &str,
    ) -> Result<SessionTurnLease> {
        let pool = self.pool().await?;
        let lease_id = loop {
            let observed = sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT lease_id, owner_pid, updated_at FROM session_turn_leases WHERE session_id = ?",
            )
            .bind(session_id)
            .fetch_optional(pool)
            .await?;
            let now = unix_timestamp();
            let owner_is_live = match observed.as_ref() {
                Some((_, owner_pid, _)) => match u32::try_from(*owner_pid) {
                    Ok(pid) => crate::subprocess::process_is_alive(pid).await,
                    Err(_) => false,
                },
                None => false,
            };
            let heartbeat_is_fresh = observed
                .as_ref()
                .is_some_and(|(_, _, updated_at)| *updated_at >= now - LEASE_TTL.as_secs() as i64);

            let write_guard = self.acquire_write_guard().await;
            let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
            let current = sqlx::query_as::<_, (String, i64, i64)>(
                "SELECT lease_id, owner_pid, updated_at FROM session_turn_leases WHERE session_id = ?",
            )
            .bind(session_id)
            .fetch_optional(&mut *tx)
            .await?;
            if current != observed {
                tx.rollback().await?;
                drop(write_guard);
                continue;
            }
            if owner_is_live && heartbeat_is_fresh {
                tx.rollback().await?;
                anyhow::bail!(
                    "session {session_id} already has an active turn in another Gosling process or window"
                );
            }
            if let Some((existing_lease_id, _, _)) = current {
                sqlx::query(
                    "DELETE FROM session_turn_leases WHERE session_id = ? AND lease_id = ?",
                )
                .bind(session_id)
                .bind(existing_lease_id)
                .execute(&mut *tx)
                .await?;
            }

            let lease_id = uuid::Uuid::new_v4().to_string();
            sqlx::query(
                r#"
                INSERT INTO session_turn_leases (
                    session_id, lease_id, owner_id, owner_pid, acquired_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(session_id)
            .bind(&lease_id)
            .bind(&self.owner_id)
            .bind(std::process::id() as i64)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            drop(write_guard);
            break lease_id;
        };

        let heartbeat_cancel = CancellationToken::new();
        let heartbeat_storage = Arc::clone(&self);
        let heartbeat_session_id = session_id.to_string();
        let heartbeat_lease_id = lease_id.clone();
        let heartbeat_stop = heartbeat_cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(LEASE_HEARTBEAT_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            interval.tick().await;
            loop {
                tokio::select! {
                    _ = heartbeat_stop.cancelled() => break,
                    _ = interval.tick() => {
                        let _ = heartbeat_storage
                            .renew_session_turn_lease(&heartbeat_session_id, &heartbeat_lease_id)
                            .await;
                    }
                }
            }
        });

        Ok(SessionTurnLease {
            storage: self,
            session_id: session_id.to_string(),
            lease_id,
            heartbeat_cancel,
            released: false,
        })
    }

    async fn renew_session_turn_lease(&self, session_id: &str, lease_id: &str) -> Result<()> {
        let _write_guard = self.acquire_write_guard().await;
        sqlx::query(
            "UPDATE session_turn_leases SET updated_at = ? WHERE session_id = ? AND lease_id = ?",
        )
        .bind(unix_timestamp())
        .bind(session_id)
        .bind(lease_id)
        .execute(self.pool().await?)
        .await?;
        Ok(())
    }

    async fn release_session_turn_lease(&self, session_id: &str, lease_id: &str) -> Result<()> {
        let _write_guard = self.acquire_write_guard().await;
        sqlx::query("DELETE FROM session_turn_leases WHERE session_id = ? AND lease_id = ?")
            .bind(session_id)
            .bind(lease_id)
            .execute(self.pool().await?)
            .await?;
        Ok(())
    }
}
