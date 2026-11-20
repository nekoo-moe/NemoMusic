//! The one function every account write goes through ([`ScClient::write`]).
//!
//! It needs a signed-in account, carries the session token to the API host only, goes through
//! the same gate as every read (one request at a time, the least gap, the 429 backoff, the
//! opt-in switch) and waits for [`WRITE_GAP`] after the last write, so clicks are paced like a
//! person's. A write is sent once: a 5xx or a lost connection is reported, never repeated,
//! because the change may have happened. A 401 or 403 drops the client id (the next call looks it
//! up again) and is reported as a refusal; ending the session stays the reads' job.

use std::time::Instant;

use super::{Call, ClientError, MAX_JSON_BYTES, Pending, PoisonError, ScClient};
use crate::write::WriteCall;

impl ScClient {
    /// Send `call` as the signed-in user and return the body of the 2xx answer.
    ///
    /// # Errors
    /// [`ClientError::NotSignedIn`] without a session, [`ClientError::Off`] while switched off,
    /// [`ClientError::RateLimited`] for a 429 (also while its backoff lasts),
    /// [`ClientError::NotFound`] for a 404 or 410, [`ClientError::Refused`] for a 401, 403 or any
    /// other 4xx, [`ClientError::Network`] and [`ClientError::Timeout`] when no answer came.
    pub async fn write(&self, call: &WriteCall) -> Result<Vec<u8>, ClientError> {
        let (token, _) = self.auth.current().ok_or(ClientError::NotSignedIn)?;
        // A person waits for the answer: background hints stand aside.
        let _pending = Pending::new(&self.interactive);
        self.check_blocked()?;
        let id = self.client_id().await?;
        let url = self.api_url(&call.path, &[], &id)?;
        let body = match &call.body {
            Some(value) => Some(serde_json::to_vec(value).map_err(|_| ClientError::Parse)?),
            None => None,
        };
        let sent = Call {
            url,
            max_bytes: MAX_JSON_BYTES,
            token: Some(&token),
            method: call.method,
            body,
        };
        let answer = self.send_paced(&sent).await?;
        match answer.status {
            200..=299 => Ok(answer.body),
            401 | 403 => {
                self.forget_id(&id).await;
                Err(ClientError::Refused)
            }
            404 | 410 => Err(ClientError::NotFound),
            500..=599 => Err(ClientError::Network),
            _ => Err(ClientError::Refused),
        }
    }

    /// Wait until [`WRITE_GAP`] has passed since the last write started, then mark this one.
    pub(super) async fn wait_for_write_turn(&self) {
        let last = *self
            .last_write
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(last) = last {
            tokio::time::sleep_until((last + self.write_gap).into()).await;
        }
        *self
            .last_write
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }
}
