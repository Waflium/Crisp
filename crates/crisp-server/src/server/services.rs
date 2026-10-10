use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use base64::Engine;
use rsa::RsaPublicKey;
use rsa::pkcs8::DecodePublicKey;

use crate::server::players::NameAndId;
use crate::util::signature_validator::SignatureValidator;

const DISCOVERY_URL: &str = "https://discovery.minecraftservices.com/minecraft/client";
const REFRESH_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
const MAX_BACKOFF_EXPONENT: u32 = 6;

struct Fetched<T> {
    value: Mutex<Option<Arc<T>>>,
    ready: (Mutex<bool>, Condvar),
    attempt: tokio::sync::Notify,
}

impl<T> Fetched<T> {
    fn new(ready: bool) -> Fetched<T> {
        Fetched { value: Mutex::new(None), ready: (Mutex::new(ready), Condvar::new()), attempt: tokio::sync::Notify::new() }
    }

    async fn get_async(&self) -> Option<Arc<T>> {
        loop {
            let attempt = self.attempt.notified();
            if *self.ready.0.lock().unwrap() {
                return self.value.lock().unwrap().clone();
            }
            attempt.await;
        }
    }

    fn get(&self) -> Option<Arc<T>> {
        let (lock, condvar) = &self.ready;
        drop(condvar.wait_while(lock.lock().unwrap(), |ready| !*ready).unwrap());
        self.value.lock().unwrap().clone()
    }

    fn attempted(&self, value: Option<T>) {
        if let Some(value) = value {
            *self.value.lock().unwrap() = Some(Arc::new(value));
        }
        let (lock, condvar) = &self.ready;
        *lock.lock().unwrap() = true;
        condvar.notify_all();
        self.attempt.notify_waiters();
    }

    fn has_value(&self) -> bool {
        self.value.lock().unwrap().is_some()
    }
}

fn retryable_fetch<T: Send + Sync + 'static, F>(target: Arc<Fetched<T>>, retry: Duration, fetch: impl Fn() -> F + Send + 'static)
where
    F: Future<Output = Option<T>> + Send,
{
    tokio::spawn(async move {
        let mut failures = 0;
        loop {
            target.attempted(fetch().await);
            let delay = if target.has_value() {
                failures = 0;
                REFRESH_INTERVAL
            } else {
                let delay = retry * (1 << failures.min(MAX_BACKOFF_EXPONENT));
                failures += 1;
                delay
            };
            tokio::time::sleep(delay).await;
        }
    });
}

struct ServicesKeySet {
    profile_keys: Vec<RsaPublicKey>,
}

#[derive(Clone)]
pub struct Services {
    discovery: Arc<Fetched<serde_json::Value>>,
    key_set: Arc<Fetched<ServicesKeySet>>,
    http: Option<(reqwest::Client, tokio::runtime::Handle)>,
}

impl Services {
    pub fn create() -> Services {
        info!("Environment: Environment[discoveryUrl={DISCOVERY_URL}, name=PROD]");
        let client = reqwest::Client::builder().timeout(Duration::from_secs(10)).build().expect("an HTTP client");
        let services = Services {
            discovery: Arc::new(Fetched::new(false)),
            key_set: Arc::new(Fetched::new(false)),
            http: Some((client.clone(), tokio::runtime::Handle::current())),
        };
        let http = client.clone();
        retryable_fetch(services.discovery.clone(), Duration::from_secs(60), move || {
            let http = http.clone();
            async move { get_json(&http, DISCOVERY_URL).await }
        });
        let this = services.clone();
        retryable_fetch(services.key_set.clone(), Duration::from_secs(5 * 60), move || {
            let (http, this) = (client.clone(), this.clone());
            async move {
                let url = this.url_async("authentication", "getPublicKeys").await?;
                let response = get_json(&http, &url).await?;
                let keys = response["playerCertificateKeys"].as_array().map(Vec::as_slice).unwrap_or_default();
                let profile_keys = keys.iter().map(|k| parse_key(k["publicKey"].as_str()?)).collect::<Option<Vec<_>>>();
                if profile_keys.is_none() {
                    error!("Received malformed Minecraft Services public key data");
                }
                Some(ServicesKeySet { profile_keys: profile_keys? })
            }
        });
        services
    }

    pub fn offline() -> Services {
        Services { discovery: Arc::new(Fetched::new(true)), key_set: Arc::new(Fetched::new(true)), http: None }
    }

    pub async fn url_async(&self, service: &str, endpoint: &str) -> Option<String> {
        let Some(discovery) = self.discovery.get_async().await else {
            warn!("Services are unavailable, unable to fetch uri for {service}/{endpoint}");
            return None;
        };
        let uri = discovery["discovery"][service]["endpoints"][endpoint]["uri"].as_str();
        if uri.is_none() {
            warn!("Services are available, but unable to find uri for {service}/{endpoint}");
        }
        uri.map(str::to_string)
    }

    pub fn profile_key_signature_validator(&self) -> Option<SignatureValidator> {
        let keys = self.key_set.get()?;
        (!keys.profile_keys.is_empty()).then(|| SignatureValidator::Services(keys.profile_keys.clone()))
    }

    pub fn find_profile_by_name(&self, name: &str) -> Option<NameAndId> {
        let (http, runtime) = self.http.as_ref()?;
        runtime.block_on(async {
            let url = self.url_async("profiles", "getByName").await?.replace("{name}", &name.to_lowercase());
            let found = match http.get(&url).send().await.and_then(reqwest::Response::error_for_status) {
                Ok(response) if response.status() == reqwest::StatusCode::OK => response.json::<serde_json::Value>().await.ok(),
                Ok(_) => None,
                Err(_) => {
                    warn!("Couldn't find profile with name: {name}");
                    None
                }
            }?;
            let id = u128::from_str_radix(found["id"].as_str()?, 16).ok()?;
            Some(NameAndId { id, name: found["name"].as_str()?.to_string() })
        })
    }

    pub fn fetch_profile(&self, id: u128) -> Option<NameAndId> {
        let (http, runtime) = self.http.as_ref()?;
        runtime.block_on(async {
            let endpoint = self.url_async("session", "getProfileById").await?.replace("{profileId}", &format!("{id:032x}"));
            let separator = if endpoint.contains('?') { '&' } else { '?' };
            let url = format!("{endpoint}{separator}unsigned=false");
            let response = match http.get(&url).send().await.and_then(reqwest::Response::error_for_status) {
                Ok(response) => response,
                Err(e) => {
                    warn!("Couldn't look up profile properties for {}\ncom.mojang.authlib.exceptions.MinecraftClientHttpException: {e}", crate::world::level::storage::format_uuid(id));
                    return None;
                }
            };
            if response.status() != reqwest::StatusCode::OK {
                return None;
            }
            let found = response.json::<serde_json::Value>().await.ok()?;
            let id = u128::from_str_radix(found["id"].as_str()?, 16).ok()?;
            Some(NameAndId { id, name: found["name"].as_str()?.to_string() })
        })
    }

    pub fn can_validate_profile_keys(&self) -> bool {
        self.key_set.get().is_some_and(|keys| !keys.profile_keys.is_empty())
    }
}

async fn get_json(http: &reqwest::Client, url: &str) -> Option<serde_json::Value> {
    match http.get(url).send().await.and_then(reqwest::Response::error_for_status) {
        Ok(response) => response.json().await.ok(),
        Err(_) => {
            warn!("Failed to connect");
            None
        }
    }
}

fn parse_key(base64: &str) -> Option<RsaPublicKey> {
    let der = base64::engine::general_purpose::STANDARD.decode(base64).ok()?;
    RsaPublicKey::from_public_key_der(&der).ok()
}
