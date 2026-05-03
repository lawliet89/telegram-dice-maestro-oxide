use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::Arc;

use rand::rngs::{StdRng, SysRng};
use rand::SeedableRng;
use tokio::sync::Mutex;

use teloxide::types::{ChatId, Message, UserId};

pub(crate) struct User {
    rng_state: StdRng,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum RngKey {
    User(UserId),
    Chat(ChatId),
}

impl User {
    fn new() -> Self {
        Self {
            rng_state: StdRng::try_from_rng(&mut SysRng).expect("SysRng infallible"),
        }
    }
}

#[derive(Clone)]
pub(crate) struct UserStore {
    // In-memory RNG state keyed by logical sender identity so each user/chat
    // sees a stable random stream across multiple rolls.
    users: Arc<Mutex<HashMap<RngKey, User>>>,
}

impl UserStore {
    pub(crate) fn new() -> Self {
        Self {
            users: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) async fn with_message_rng<F, T>(&self, msg: &Message, f: F) -> T
    where
        F: FnOnce(&mut StdRng) -> T,
    {
        self.with_rng(Self::rng_key(msg), f).await
    }

    async fn with_rng<F, T>(&self, key: RngKey, f: F) -> T
    where
        F: FnOnce(&mut StdRng) -> T,
    {
        let mut guard = self.users.lock().await;
        let user = match guard.entry(key) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => {
                log::info!("Created RNG state for {:?}", key);
                e.insert(User::new())
            }
        };
        f(&mut user.rng_state)
    }

    fn rng_key(msg: &Message) -> RngKey {
        // Most messages have a concrete user sender. For sender-less messages
        // such as channel posts, fall back to chat identity so unrelated chats
        // do not share RNG state.
        match msg.from.as_ref().map(|u| u.id) {
            Some(user_id) => RngKey::User(user_id),
            None => RngKey::Chat(msg.chat.id),
        }
    }

    #[cfg(test)]
    async fn len(&self) -> usize {
        self.users.lock().await.len()
    }

    #[cfg(test)]
    async fn contains_key(&self, key: RngKey) -> bool {
        self.users.lock().await.contains_key(&key)
    }

    #[cfg(test)]
    async fn insert(&self, key: RngKey, user: User) {
        self.users.lock().await.insert(key, user);
    }
}

#[cfg(test)]
impl User {
    fn with_rng(rng: StdRng) -> Self {
        Self { rng_state: rng }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;

    // Multiple calls for the same user_id create exactly one entry in the store,
    // not a new entry per call.
    #[tokio::test]
    async fn same_user_creates_single_store_entry() {
        let store = UserStore::new();
        let uid = RngKey::User(UserId(42));

        store.with_rng(uid, |_| {}).await;
        store.with_rng(uid, |_| {}).await;

        assert_eq!(store.len().await, 1);
    }

    // Two distinct user_ids each create their own entry — the store holds exactly
    // two entries after both users roll, one per user.
    #[tokio::test]
    async fn different_users_create_separate_store_entries() {
        let store = UserStore::new();
        store.with_rng(RngKey::User(UserId(1)), |_| {}).await;
        store.with_rng(RngKey::User(UserId(2)), |_| {}).await;

        assert_eq!(store.len().await, 2);
        assert!(store.contains_key(RngKey::User(UserId(1))).await);
        assert!(store.contains_key(RngKey::User(UserId(2))).await);
    }

    // Messages without a sender fall back to chat identity.
    #[tokio::test]
    async fn chat_fallback_works() {
        let store = UserStore::new();
        store.with_rng(RngKey::Chat(ChatId(7)), |_| {}).await;

        assert_eq!(store.len().await, 1);
        assert!(store.contains_key(RngKey::Chat(ChatId(7))).await);
    }

    #[tokio::test]
    async fn different_fallback_chats_create_separate_store_entries() {
        let store = UserStore::new();
        store.with_rng(RngKey::Chat(ChatId(1)), |_| {}).await;
        store.with_rng(RngKey::Chat(ChatId(2)), |_| {}).await;

        assert_eq!(store.len().await, 2);
        assert!(store.contains_key(RngKey::Chat(ChatId(1))).await);
        assert!(store.contains_key(RngKey::Chat(ChatId(2))).await);
    }

    // The same user's RNG state advances with each call: two consecutive rolls
    // consume consecutive outputs of the same persisted RNG, not independent
    // fresh RNGs seeded anew on every call.
    #[tokio::test]
    async fn same_user_rng_state_advances_across_calls() {
        let store = UserStore::new();
        let uid = RngKey::User(UserId(99));

        // Pre-seed the user's RNG with a known value so we can predict outputs.
        store
            .insert(uid, User::with_rng(StdRng::seed_from_u64(0)))
            .await;

        // Compute the expected first two outputs from the same seed.
        let mut reference = StdRng::seed_from_u64(0);
        let expected1 = reference.next_u64();
        let expected2 = reference.next_u64();

        // Each call must advance the same shared RNG, not create a fresh one.
        let actual1 = store.with_rng(uid, |rng| rng.next_u64()).await;
        let actual2 = store.with_rng(uid, |rng| rng.next_u64()).await;

        assert_eq!(actual1, expected1);
        assert_eq!(actual2, expected2);
    }
}
