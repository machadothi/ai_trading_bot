use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::str::FromStr;
use tracing::{debug, info};

/// CoinGecko API client for fetching crypto market data
pub struct CoinGeckoClient {
    client: reqwest::Client,
    base_url: String,
}

/// Hourly OHLC data point
#[derive(Debug, Clone)]
pub struct OhlcData {
    pub timestamp: i64,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
}

/// Market data fetched from CoinGecko
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CoinGeckoMarketData {
    pub symbol: String,
    pub current_price: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    pub price_change_24h_percent: Decimal,
    pub market_cap: Decimal,
    pub total_volume: Decimal,
    pub hourly_data_12h: Vec<OhlcData>,
    pub hourly_data_24h: Vec<OhlcData>,
    pub hourly_data_48h: Vec<OhlcData>,
}

/// Support and resistance levels calculated from historical data
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SupportResistanceLevels {
    pub strong_support: Decimal,
    pub support: Decimal,
    pub current_price: Decimal,
    pub resistance: Decimal,
    pub strong_resistance: Decimal,
    pub pivot_point: Decimal,
    pub timeframe: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CoinGeckoPrice {
    #[serde(default)]
    bitcoin: Option<PriceData>,
    #[serde(default)]
    ethereum: Option<PriceData>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct PriceData {
    usd: f64,
    usd_24h_change: Option<f64>,
    usd_24h_vol: Option<f64>,
    usd_market_cap: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct MarketChartResponse {
    prices: Vec<Vec<f64>>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct CoinMarketData {
    id: String,
    symbol: String,
    current_price: f64,
    high_24h: Option<f64>,
    low_24h: Option<f64>,
    price_change_percentage_24h: Option<f64>,
    market_cap: Option<f64>,
    total_volume: Option<f64>,
}

impl CoinGeckoClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("CryptoTradingBot/1.0 (Rust; Educational Project)")
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://api.coingecko.com/api/v3".to_string(),
        }
    }

    /// Map trading symbol to CoinGecko coin ID
    fn symbol_to_coin_id(symbol: &str) -> &str {
        match symbol.to_uppercase().as_str() {
            "BTCUSDT" | "BTC" | "BTCUSD" => "bitcoin",
            "ETHUSDT" | "ETH" | "ETHUSD" => "ethereum",
            "BNBUSDT" | "BNB" => "binancecoin",
            "XRPUSDT" | "XRP" => "ripple",
            "ADAUSDT" | "ADA" => "cardano",
            "SOLUSDT" | "SOL" => "solana",
            "DOTUSDT" | "DOT" => "polkadot",
            "DOGEUSDT" | "DOGE" => "dogecoin",
            "MATICUSDT" | "MATIC" => "matic-network",
            "LTCUSDT" | "LTC" => "litecoin",
            "AVAXUSDT" | "AVAX" => "avalanche-2",
            "LINKUSDT" | "LINK" => "chainlink",
            "ATOMUSDT" | "ATOM" => "cosmos",
            "UNIUSDT" | "UNI" => "uniswap",
            "XLMUSDT" | "XLM" => "stellar",
            _ => "bitcoin", // Default to bitcoin
        }
    }

    /// Fetch comprehensive market data including hourly OHLC
    pub async fn fetch_market_data(&self, symbol: &str) -> Result<CoinGeckoMarketData> {
        let coin_id = Self::symbol_to_coin_id(symbol);
        info!("Fetching CoinGecko data for {} ({})", symbol, coin_id);

        // Fetch current market data
        let market_url = format!(
            "{}/coins/markets?vs_currency=usd&ids={}&order=market_cap_desc&sparkline=false",
            self.base_url, coin_id
        );

        let response = self.client
            .get(&market_url)
            .header("Accept", "application/json")
            .send()
            .await?;
        
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("CoinGecko API error {}: {}", status, body));
        }
        
        let market_response: Vec<CoinMarketData> = response.json().await?;

        let market = market_response
            .first()
            .ok_or_else(|| anyhow!("No market data found for {}", coin_id))?;

        // Fetch hourly data for different timeframes
        // CoinGecko free API: 1-90 days = hourly data
        let hourly_48h = self.fetch_hourly_prices(coin_id, 2).await?;
        
        // Split into timeframes
        let hourly_24h: Vec<OhlcData> = hourly_48h.iter()
            .rev()
            .take(24)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        let hourly_12h: Vec<OhlcData> = hourly_48h.iter()
            .rev()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        Ok(CoinGeckoMarketData {
            symbol: symbol.to_string(),
            current_price: Decimal::from_str(&market.current_price.to_string())?,
            high_24h: Decimal::from_str(&market.high_24h.unwrap_or(market.current_price).to_string())?,
            low_24h: Decimal::from_str(&market.low_24h.unwrap_or(market.current_price).to_string())?,
            price_change_24h_percent: Decimal::from_str(
                &market.price_change_percentage_24h.unwrap_or(0.0).to_string()
            )?,
            market_cap: Decimal::from_str(&market.market_cap.unwrap_or(0.0).to_string())?,
            total_volume: Decimal::from_str(&market.total_volume.unwrap_or(0.0).to_string())?,
            hourly_data_12h: hourly_12h,
            hourly_data_24h: hourly_24h,
            hourly_data_48h: hourly_48h,
        })
    }

    /// Fetch hourly price data for a given number of days
    async fn fetch_hourly_prices(&self, coin_id: &str, days: u32) -> Result<Vec<OhlcData>> {
        let url = format!(
            "{}/coins/{}/market_chart?vs_currency=usd&days={}",
            self.base_url, coin_id, days
        );

        debug!("Fetching hourly data: {}", url);

        let response = self.client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?;
            
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(anyhow!("CoinGecko chart API error {}: {}", status, body));
        }
        
        let chart_data: MarketChartResponse = response.json().await?;

        // Convert price data to OHLC format
        // CoinGecko returns [timestamp_ms, price] pairs
        // We'll create pseudo-OHLC from consecutive prices
        let mut ohlc_data: Vec<OhlcData> = Vec::new();
        
        for window in chart_data.prices.windows(2) {
            if let [current, next] = window {
                let timestamp = current[0] as i64;
                let price1 = Decimal::from_str(&current[1].to_string())?;
                let price2 = Decimal::from_str(&next[1].to_string())?;
                
                ohlc_data.push(OhlcData {
                    timestamp: timestamp / 1000, // Convert to seconds
                    open: price1,
                    high: price1.max(price2),
                    low: price1.min(price2),
                    close: price2,
                });
            }
        }

        // Add the last data point
        if let Some(last) = chart_data.prices.last() {
            let price = Decimal::from_str(&last[1].to_string())?;
            ohlc_data.push(OhlcData {
                timestamp: last[0] as i64 / 1000,
                open: price,
                high: price,
                low: price,
                close: price,
            });
        }

        info!("Fetched {} hourly data points for {}", ohlc_data.len(), coin_id);
        Ok(ohlc_data)
    }

    /// Calculate support and resistance levels using pivot points
    #[allow(dead_code)]
    pub fn calculate_support_resistance(
        &self,
        data: &[OhlcData],
        current_price: Decimal,
        timeframe: &str,
    ) -> SupportResistanceLevels {
        if data.is_empty() {
            return SupportResistanceLevels {
                strong_support: current_price * Decimal::from_str("0.95").unwrap(),
                support: current_price * Decimal::from_str("0.97").unwrap(),
                current_price,
                resistance: current_price * Decimal::from_str("1.03").unwrap(),
                strong_resistance: current_price * Decimal::from_str("1.05").unwrap(),
                pivot_point: current_price,
                timeframe: timeframe.to_string(),
            };
        }

        // Calculate high, low, close from the period
        let high = data.iter().map(|d| d.high).max().unwrap_or(current_price);
        let low = data.iter().map(|d| d.low).min().unwrap_or(current_price);
        let close = data.last().map(|d| d.close).unwrap_or(current_price);

        // Calculate pivot point (standard formula)
        let pivot = (high + low + close) / Decimal::from(3);

        // Calculate support and resistance levels
        // R1 = 2 * Pivot - Low
        // R2 = Pivot + (High - Low)
        // S1 = 2 * Pivot - High
        // S2 = Pivot - (High - Low)
        let range = high - low;
        
        let resistance1 = Decimal::from(2) * pivot - low;
        let resistance2 = pivot + range;
        let support1 = Decimal::from(2) * pivot - high;
        let support2 = pivot - range;

        SupportResistanceLevels {
            strong_support: support2,
            support: support1,
            current_price,
            resistance: resistance1,
            strong_resistance: resistance2,
            pivot_point: pivot,
            timeframe: timeframe.to_string(),
        }
    }

    /// Find key price levels from historical data (local highs/lows)
    #[allow(dead_code)]
    pub fn find_key_levels(&self, data: &[OhlcData]) -> (Vec<Decimal>, Vec<Decimal>) {
        let mut support_levels: Vec<Decimal> = Vec::new();
        let mut resistance_levels: Vec<Decimal> = Vec::new();

        if data.len() < 3 {
            return (support_levels, resistance_levels);
        }

        // Find local minima (support) and maxima (resistance)
        for i in 1..data.len() - 1 {
            let prev = &data[i - 1];
            let curr = &data[i];
            let next = &data[i + 1];

            // Local minimum (support)
            if curr.low < prev.low && curr.low < next.low {
                support_levels.push(curr.low);
            }

            // Local maximum (resistance)
            if curr.high > prev.high && curr.high > next.high {
                resistance_levels.push(curr.high);
            }
        }

        // Sort and deduplicate
        support_levels.sort();
        resistance_levels.sort();

        (support_levels, resistance_levels)
    }

    /// Format hourly data for AI analysis
    pub fn format_for_ai(&self, data: &CoinGeckoMarketData) -> String {
        let mut output = String::new();
        
        output.push_str(&format!("=== {} Market Analysis ===\n\n", data.symbol));
        output.push_str(&format!("Current Price: ${:.2}\n", data.current_price));
        output.push_str(&format!("24h High: ${:.2}\n", data.high_24h));
        output.push_str(&format!("24h Low: ${:.2}\n", data.low_24h));
        output.push_str(&format!("24h Change: {:.2}%\n", data.price_change_24h_percent));
        output.push_str(&format!("24h Volume: ${:.0}\n\n", data.total_volume));

        // 12h summary
        if !data.hourly_data_12h.is_empty() {
            let high_12h = data.hourly_data_12h.iter().map(|d| d.high).max().unwrap();
            let low_12h = data.hourly_data_12h.iter().map(|d| d.low).min().unwrap();
            output.push_str("=== Last 12 Hours ===\n");
            output.push_str(&format!("High: ${:.2}, Low: ${:.2}\n", high_12h, low_12h));
            output.push_str(&format!("Range: ${:.2}\n\n", high_12h - low_12h));
        }

        // 24h summary  
        if !data.hourly_data_24h.is_empty() {
            let high_24h = data.hourly_data_24h.iter().map(|d| d.high).max().unwrap();
            let low_24h = data.hourly_data_24h.iter().map(|d| d.low).min().unwrap();
            output.push_str("=== Last 24 Hours ===\n");
            output.push_str(&format!("High: ${:.2}, Low: ${:.2}\n", high_24h, low_24h));
            output.push_str(&format!("Range: ${:.2}\n\n", high_24h - low_24h));
        }

        // 48h summary
        if !data.hourly_data_48h.is_empty() {
            let high_48h = data.hourly_data_48h.iter().map(|d| d.high).max().unwrap();
            let low_48h = data.hourly_data_48h.iter().map(|d| d.low).min().unwrap();
            output.push_str("=== Last 48 Hours ===\n");
            output.push_str(&format!("High: ${:.2}, Low: ${:.2}\n", high_48h, low_48h));
            output.push_str(&format!("Range: ${:.2}\n\n", high_48h - low_48h));

            // Include hourly prices for AI
            output.push_str("=== Hourly Prices (48h) ===\n");
            for (i, ohlc) in data.hourly_data_48h.iter().enumerate() {
                if i % 4 == 0 { // Every 4 hours to keep it concise
                    let dt = chrono::DateTime::from_timestamp(ohlc.timestamp, 0)
                        .map(|d| d.format("%m/%d %H:%M").to_string())
                        .unwrap_or_default();
                    output.push_str(&format!(
                        "{}: O=${:.2} H=${:.2} L=${:.2} C=${:.2}\n",
                        dt, ohlc.open, ohlc.high, ohlc.low, ohlc.close
                    ));
                }
            }
        }

        output
    }
}

impl Default for CoinGeckoClient {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_mapping() {
        assert_eq!(CoinGeckoClient::symbol_to_coin_id("BTCUSDT"), "bitcoin");
        assert_eq!(CoinGeckoClient::symbol_to_coin_id("ETHUSDT"), "ethereum");
        assert_eq!(CoinGeckoClient::symbol_to_coin_id("btc"), "bitcoin");
    }
}
