use std::collections::HashMap;
use std::sync::Arc;

use rand::rngs::{OsRng, StdRng};
use rand::SeedableRng;
use tokio::sync::Mutex;

use teloxide::types::UserId;

pub struct User {
    pub rng_state: StdRng,
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
    let user = guard.entry(user_id).or_insert_with(User::new);
    f(&mut user.rng_state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[tokio::test]
    async fn same_user_shares_rng_state() {
        let store = new_store();
        let uid = Some(UserId(42));

        let first = with_user_rng(&store, uid, |rng| rng.next_u64()).await;
        let second = with_user_rng(&store, uid, |rng| rng.next_u64()).await;

        assert_ne!(first, second);

        let store2 = new_store();
        let alt = with_user_rng(&store2, uid, |rng| rng.next_u64()).await;
        assert_ne!(first, alt);
    }

    #[tokio::test]
    async fn different_users_have_independent_rng() {
        let store = new_store();
        let a = with_user_rng(&store, Some(UserId(1)), |rng| rng.next_u64()).await;
        let b = with_user_rng(&store, Some(UserId(2)), |rng| rng.next_u64()).await;
        assert_ne!(a, b);
    }

    #[tokio::test]
    async fn anonymous_user_works() {
        let store = new_store();
        let v = with_user_rng(&store, None, |rng| rng.next_u64()).await;
        assert_ne!(v, 0);
    }
}
