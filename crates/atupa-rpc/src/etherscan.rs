use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;
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

/// A lightweight client to resolve EVM addresses into Human-Readable Contract Names.
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
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            api_key,
            chain_id,
        }
    }

    /// Resolves an address to its verified Contract Name via Etherscan.
    #[allow(clippy::collapsible_if)]
    pub async fn resolve_contract_name(&self, address: &str) -> Option<String> {
        if address.len() < 40 {
            return None;
        }

        // Fast local hit
        {
            let cache_lock = self.cache.lock().await;
            if let Some(name) = cache_lock.get(address) {
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
                        let mut cache_lock = self.cache.lock().await;
                        cache_lock.insert(address.to_string(), name.clone());
                        return Some(name);
                    }
                    _ => {
                        log::debug!("❌ Etherscan hit but no name for {}: {:?}", address, api_res);
                    }
                }
            } else {
                log::debug!("❌ Etherscan JSON parse failed for {}", address);
            }
        }

        None
    }
}
