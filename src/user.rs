use std::collections::HashMap;
use std::sync::Arc;

use rand::rngs::{OsRng, StdRng};
use rand::SeedableRng;
use tokio::sync::Mutex;

use teloxide::types::UserId;

pub(crate) struct User {
    rng_state: StdRng,
}

impl User {
    fn new() -> Self {
        Self {
            rng_state: StdRng::from_rng(OsRng).expect("OsRng infallible"),
        }
    }
}

pub(crate) type UserStore = Arc<Mutex<HashMap<Option<UserId>, User>>>;

pub(crate) fn new_store() -> UserStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub(crate) async fn with_user_rng<F, T>(store: &UserStore, user_id: Option<UserId>, f: F) -> T
where
    F: FnOnce(&mut StdRng) -> T,
{
    let mut guard = store.lock().await;
    let occupied = guard.contains_key(&user_id);
    let user = guard.entry(user_id).or_insert_with(User::new);
    if !occupied {
        log::info!("Created RNG state for user {:?}", user_id);
    }
    f(&mut user.rng_state)
}

#[cfg(test)]
impl User {
    fn with_rng(rng: StdRng) -> Self {
        Self { rng_state: rng }
    }
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use super::*;

    // Multiple calls for the same user_id create exactly one entry in the store,
    // not a new entry per call.
    #[tokio::test]
    async fn same_user_creates_single_store_entry() {
        let store = new_store();
        let uid = Some(UserId(42));

        with_user_rng(&store, uid, |_| {}).await;
        with_user_rng(&store, uid, |_| {}).await;

        assert_eq!(store.lock().await.len(), 1);
    }

    // Two distinct user_ids each create their own entry — the store holds exactly
    // two entries after both users roll, one per user.
    #[tokio::test]
    async fn different_users_create_separate_store_entries() {
        let store = new_store();
        with_user_rng(&store, Some(UserId(1)), |_| {}).await;
        with_user_rng(&store, Some(UserId(2)), |_| {}).await;

        let guard = store.lock().await;
        assert_eq!(guard.len(), 2);
        assert!(guard.contains_key(&Some(UserId(1))));
        assert!(guard.contains_key(&Some(UserId(2))));
    }

    // None is a valid key and gets its own entry.
    #[tokio::test]
    async fn anonymous_user_works() {
        let store = new_store();
        with_user_rng(&store, None, |_| {}).await;

        let guard = store.lock().await;
        assert_eq!(guard.len(), 1);
        assert!(guard.contains_key(&None));
    }

    // The same user's RNG state advances with each call: two consecutive rolls
    // consume consecutive outputs of the same persisted RNG, not independent
    // fresh RNGs seeded anew on every call.
    #[tokio::test]
    async fn same_user_rng_state_advances_across_calls() {
        let store = new_store();
        let uid = Some(UserId(99));

        // Pre-seed the user's RNG with a known value so we can predict outputs.
        store
            .lock()
            .await
            .insert(uid, User::with_rng(StdRng::seed_from_u64(0)));

        // Compute the expected first two outputs from the same seed.
        let mut reference = StdRng::seed_from_u64(0);
        let expected1 = reference.next_u64();
        let expected2 = reference.next_u64();

        // Each call must advance the same shared RNG, not create a fresh one.
        let actual1 = with_user_rng(&store, uid, |rng| rng.next_u64()).await;
        let actual2 = with_user_rng(&store, uid, |rng| rng.next_u64()).await;

        assert_eq!(actual1, expected1);
        assert_eq!(actual2, expected2);
    }
}
