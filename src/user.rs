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

pub type UserStore = Arc<Mutex<HashMap<Option<UserId>, User>>>;

pub fn new_store() -> UserStore {
    Arc::new(Mutex::new(HashMap::new()))
}

pub async fn with_user_rng<F, T>(store: &UserStore, user_id: Option<UserId>, f: F) -> T
where
    F: FnOnce(&mut StdRng) -> T,
{
    let mut guard = store.lock().await;
    let occupied = guard.contains_key(&user_id);
    let user = guard.entry(user_id).or_insert_with(User::new);
    if occupied {
        log::info!("Fetched RNG state for user {:?}", user_id);
    } else {
        log::info!("Created RNG state for user {:?}", user_id);
    }
    f(&mut user.rng_state)
}

#[cfg(test)]
mod tests {
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

    // Two distinct user_ids produce two separate map entries.
    #[tokio::test]
    async fn different_users_have_independent_rng() {
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
}
