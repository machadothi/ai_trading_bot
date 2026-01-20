use crate::config::Config;
use crate::models::{Balance, OrderSide, OrderType, Order};
use anyhow::Result;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal;
use sha2::Sha256;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

pub struct ExchangeClient {
    config: Config,
    client: reqwest::Client,
}

impl ExchangeClient {
    pub async fn new(config: &Config) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;

        Ok(Self {
            config: config.clone(),
            client,
        })
    }

    fn sign(&self, query_string: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    fn timestamp() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    pub async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let url = format!("{}/api/v3/ticker/price?symbol={}", self.config.base_url, symbol);
        
        let response: serde_json::Value = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let price_str = response["price"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Price not found in response"))?;

        Ok(price_str.parse()?)
    }

    pub async fn get_balance(&self) -> Result<HashMap<String, Balance>> {
        let timestamp = Self::timestamp();
        let query = format!("timestamp={}", timestamp);
        let signature = self.sign(&query);

        let url = format!(
            "{}/api/v3/account?{}&signature={}",
            self.config.base_url, query, signature
        );

        let response: serde_json::Value = self.client
            .get(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?
            .json()
            .await?;

        let mut balances = HashMap::new();
        
        if let Some(balance_array) = response["balances"].as_array() {
            for b in balance_array {
                let asset = b["asset"].as_str().unwrap_or_default().to_string();
                let free: Decimal = b["free"].as_str().unwrap_or("0").parse().unwrap_or_default();
                let locked: Decimal = b["locked"].as_str().unwrap_or("0").parse().unwrap_or_default();
                
                if free > Decimal::ZERO || locked > Decimal::ZERO {
                    balances.insert(asset.clone(), Balance { asset, free, locked });
                }
            }
        }

        Ok(balances)
    }

    #[allow(dead_code)]
    pub async fn place_order(
        &self,
        symbol: &str,
        side: OrderSide,
        order_type: OrderType,
        quantity: Decimal,
        price: Option<Decimal>,
    ) -> Result<Order> {
        let timestamp = Self::timestamp();
        
        let mut params = vec![
            format!("symbol={}", symbol),
            format!("side={}", side),
            format!("type={}", order_type),
            format!("quantity={}", quantity),
            format!("timestamp={}", timestamp),
        ];

        if let Some(p) = price {
            params.push(format!("price={}", p));
            params.push("timeInForce=GTC".to_string());
        }

        let query = params.join("&");
        let signature = self.sign(&query);
        
        let url = format!(
            "{}/api/v3/order?{}&signature={}",
            self.config.base_url, query, signature
        );

        let response: Order = self.client
            .post(&url)
            .header("X-MBX-APIKEY", &self.config.api_key)
            .send()
            .await?
            .json()
            .await?;

        Ok(response)
    }

    #[allow(dead_code)]
    pub async fn get_klines(
        &self,
        symbol: &str,
        interval: &str,
        limit: u32,
    ) -> Result<Vec<crate::models::Kline>> {
        let url = format!(
            "{}/api/v3/klines?symbol={}&interval={}&limit={}",
            self.config.base_url, symbol, interval, limit
        );

        let response: Vec<Vec<serde_json::Value>> = self.client
            .get(&url)
            .send()
            .await?
            .json()
            .await?;

        let klines = response
            .into_iter()
            .map(|k| crate::models::Kline {
                open_time: k[0].as_i64().unwrap_or_default(),
                open: k[1].as_str().unwrap_or("0").parse().unwrap_or_default(),
                high: k[2].as_str().unwrap_or("0").parse().unwrap_or_default(),
                low: k[3].as_str().unwrap_or("0").parse().unwrap_or_default(),
                close: k[4].as_str().unwrap_or("0").parse().unwrap_or_default(),
                volume: k[5].as_str().unwrap_or("0").parse().unwrap_or_default(),
                close_time: k[6].as_i64().unwrap_or_default(),
            })
            .collect();

        Ok(klines)
    }
}
