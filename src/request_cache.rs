use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone)]
struct Value<T> {
    value: Arc<T>,
    inserted: DateTime<Utc>,
    expires: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct RequestCache<T> {
    map: Arc<RwLock<HashMap<String, Value<T>>>>,
    timeout: Duration,
}

impl<T> Default for RequestCache<T> {
    fn default() -> Self {
        Self {
            timeout: Duration::minutes(1),
            map: Arc::<RwLock<HashMap<String, Value<T>>>>::default(),
        }
    }
}

impl<T> RequestCache<T> {
    pub fn new(timeout: Duration) -> RequestCache<T> {
        RequestCache {
            timeout,
            ..Default::default()
        }
    }

    pub fn get<S>(&self, key: S) -> Option<Arc<T>>
    where
        S: Into<String>,
    {
        let key: String = key.into();
        if let Some(v) = self.map.read().unwrap().get(&key) {
            if v.expires >= Utc::now() {
                return Some(v.value.clone());
            }
        }
        None
    }

    pub fn insert<S>(&self, key: S, value: T, expires: DateTime<Utc>)
    where
        S: Into<String>,
    {
        let now = Utc::now();
        if expires <= now {
            return;
        }
        let value = Value {
            value: Arc::new(value),
            inserted: now,
            expires,
        };
        self.map.write().unwrap().insert(key.into(), value);
    }

    pub fn insert_with_timeout<S>(&self, key: S, value: T, timeout: Duration)
    where
        S: Into<String>,
    {
        self.insert(key, value, Utc::now() + timeout);
    }

    pub fn insert_with_default_timeout<S>(&self, key: S, value: T)
    where
        S: Into<String>,
    {
        self.insert_with_timeout(key, value, self.timeout);
    }

    pub fn extend<S>(&self, key: S, extension: Duration)
    where
        S: Into<String>,
    {
        let mut map = self.map.write().unwrap();
        if let Some(v) = map.get_mut(&key.into()) {
            v.expires += extension;
        }
    }

    pub fn invalidate<S>(&self, key: S)
    where
        S: Into<String>,
    {
        self.map.write().unwrap().remove(&key.into());
    }

    pub fn invalidate_if_newer<S>(&self, key: S, last_update: DateTime<Utc>)
    where
        S: Into<String>,
    {
        let key = key.into();
        let mut map = self.map.write().unwrap();
        if let Some(v) = map.get(&key) {
            if v.inserted < last_update {
                map.remove(&key);
            }
        }
    }

    pub fn invalidate_all(&self) {
        self.map.write().unwrap().clear()
    }

    pub fn invalidate_expired(&self) {
        let mut map = self.map.write().unwrap();
        let now = Utc::now();
        map.retain(|_, v| v.expires > now);
    }
}
