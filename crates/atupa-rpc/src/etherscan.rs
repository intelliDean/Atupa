use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Deserialize, Debug)]
struct EtherscanResponse {
    status: String,
    result: Vec<EtherscanContractItem>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct EtherscanContractItem {
    contract_name: String,
}

/// Returns the path to the persistent Etherscan cache file.
/// Resolves to `~/.atupa/etherscan_cache.json` (or OS equivalent).
fn cache_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".atupa").join("etherscan_cache.json"))
}

/// Asynchronously loads the serialized cache from disk.
/// Returns an empty map if the file doesn't exist or cannot be parsed — non-fatal.
async fn load_cache_async() -> HashMap<String, String> {
    let Some(path) = cache_path() else {
        return HashMap::new();
    };
    match tokio::fs::read_to_string(&path).await {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => HashMap::new(),
    }
}

/// Spawns a background task to flush the cache to disk.
/// Takes an owned snapshot of the cache so the caller can drop the Mutex lock immediately.
/// Errors are logged but never propagated — a cache write failure is non-fatal.
fn spawn_flush_cache(snapshot: HashMap<String, String>) {
    tokio::spawn(async move {
        let Some(path) = cache_path() else { return };

        if let Some(parent) = path.parent()
            && let Err(e) = tokio::fs::create_dir_all(parent).await
        {
            log::warn!("⚠️  Could not create cache dir {:?}: {}", parent, e);
            return;
        }

        match serde_json::to_string_pretty(&snapshot) {
            Ok(json) => {
                if let Err(e) = tokio::fs::write(&path, json).await {
                    log::warn!("⚠️  Could not write Etherscan cache to {:?}: {}", path, e);
                }
            }
            Err(e) => {
                log::warn!("⚠️  Could not serialize Etherscan cache: {}", e);
            }
        }
    });
}

/// A lightweight client to resolve EVM addresses into Human-Readable Contract Names.
/// Resolves are cached in memory during execution and persisted to
/// `~/.atupa/etherscan_cache.json` across sessions.
#[derive(Clone)]
pub struct EtherscanResolver {
    client: Client,
    pub cache: Arc<Mutex<HashMap<String, String>>>,
    api_key: Option<String>,
    chain_id: u64,
}

impl Default for EtherscanResolver {
    fn default() -> Self {
        Self::new(None, 1) // Default to Ethereum mainnet if not specified
    }
}

impl EtherscanResolver {
    pub fn new(api_key: Option<String>, chain_id: u64) -> Self {
        // Start with an empty in-memory cache; a background task will warm it
        // from disk without blocking the Tokio executor.
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let cache_clone = cache.clone();
        tokio::spawn(async move {
            let disk_cache = load_cache_async().await;
            let cache_size = disk_cache.len();
            let mut lock = cache_clone.lock().await;
            *lock = disk_cache;
            if cache_size > 0 {
                log::info!(
                    "📦 Loaded {} Etherscan contract name(s) from disk cache",
                    cache_size
                );
            }
        });

        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            cache,
            api_key,
            chain_id,
        }
    }

    /// Resolves an address to its verified Contract Name via Etherscan.
    /// Results are cached in memory and on disk to avoid redundant API calls.
    #[allow(clippy::collapsible_if)]
    pub async fn resolve_contract_name(&self, address: &str) -> Option<String> {
        if address.len() < 40 {
            return None;
        }

        // Fast local hit (in-memory, pre-warmed from disk)
        {
            let cache_lock = self.cache.lock().await;
            if let Some(name) = cache_lock.get(address) {
                log::debug!("📦 Cache hit: {} -> {}", address, name);
                return Some(name.clone());
            }
        }

        // Network fetch (Etherscan API V2 requires chainid)
        let mut url_str = format!(
            "https://api.etherscan.io/v2/api?chainid={}&module=contract&action=getsourcecode&address={}",
            self.chain_id, address
        );
        if let Some(key) = &self.api_key {
            url_str.push_str(&format!("&apikey={}", key));
        }

        let Ok(url) = Url::parse(&url_str) else {
            return None;
        };

        if let Ok(resp) = self.client.get(url).send().await {
            if let Ok(api_res) = resp.json::<EtherscanResponse>().await {
                match (api_res.status.as_str(), api_res.result.first()) {
                    ("1", Some(item)) if !item.contract_name.is_empty() => {
                        let name = item.contract_name.clone();
                        log::info!("✅ Etherscan resolved {} -> {}", address, name);

                        // Update in-memory cache, then flush to disk in a background
                        // task — the Mutex lock is dropped before any I/O begins.
                        let mut cache_lock = self.cache.lock().await;
                        cache_lock.insert(address.to_string(), name.clone());
                        let snapshot = cache_lock.clone();
                        drop(cache_lock);
                        spawn_flush_cache(snapshot);

                        return Some(name);
                    }
                    _ => {
                        log::debug!(
                            "❌ Etherscan hit but no name for {}: {:?}",
                            address,
                            api_res
                        );
                    }
                }
            } else {
                log::debug!("❌ Etherscan JSON parse failed for {}", address);
            }
        }

        None
    }
}
