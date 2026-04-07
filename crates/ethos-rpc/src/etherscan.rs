use reqwest::{Client, Url};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::time::Duration;

#[derive(Deserialize, Debug)]
struct EtherscanResponse {
    status: String,
    _message: String,
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
}

impl Default for EtherscanResolver {
    fn default() -> Self {
        Self::new(None)
    }
}

impl EtherscanResolver {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            cache: Arc::new(Mutex::new(HashMap::new())),
            api_key,
        }
    }
    
    /// Resolves an address to its verified Contract Name via Etherscan.
    pub async fn resolve_contract_name(&self, address: &str) -> Option<String> {
        if address.len() < 40 { return None; }
        
        // Fast local hit
        {
            let cache_lock = self.cache.lock().await;
            if let Some(name) = cache_lock.get(address) {
                return Some(name.clone());
            }
        }
        
        // Network fetch (Etherscan API V2 requires chainid)
        let mut url_str = format!("https://api.etherscan.io/v2/api?chainid=1&module=contract&action=getsourcecode&address={}", address);
        if let Some(key) = &self.api_key {
            url_str.push_str(&format!("&apikey={}", key));
        }
        
        let Ok(url) = Url::parse(&url_str) else { return None; };
        
        if let Ok(resp) = self.client.get(url).send().await {
            if let Ok(api_res) = resp.json::<EtherscanResponse>().await {
                if api_res.status == "1" && !api_res.result.is_empty() {
                    let name = api_res.result[0].contract_name.clone();
                    if !name.is_empty() {
                        let mut cache_lock = self.cache.lock().await;
                        cache_lock.insert(address.to_string(), name.clone());
                        return Some(name);
                    }
                }
            }
        }
        
        None
    }
}
