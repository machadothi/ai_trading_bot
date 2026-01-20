use anyhow::{anyhow, Result};

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct Config {
    pub exchange: String,
    pub api_key: String,
    pub api_secret: String,
    pub symbol: String,
    pub base_url: String,
    pub ws_url: String,
    pub simulation_mode: bool,
    pub simulation_initial_balance: rust_decimal::Decimal,
    pub simulation_price_volatility: f64,
    pub report_path: String,
    pub stop_loss_percent: rust_decimal::Decimal,
    pub take_profit_percent: rust_decimal::Decimal,
    // AI/Ollama settings
    pub ollama_enabled: bool,
    pub ollama_url: String,
    pub ollama_model: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let exchange = std::env::var("EXCHANGE").unwrap_or_else(|_| "binance".to_string());
        let simulation_mode = std::env::var("SIMULATION_MODE")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);
        
        let (base_url, ws_url) = match exchange.as_str() {
            "binance" => (
                "https://api.binance.com".to_string(),
                "wss://stream.binance.com:9443/ws".to_string(),
            ),
            "binance_testnet" => (
                "https://testnet.binance.vision".to_string(),
                "wss://testnet.binance.vision/ws".to_string(),
            ),
            "simulation" => (
                "simulation".to_string(),
                "simulation".to_string(),
            ),
            _ => return Err(anyhow!("Unsupported exchange: {}", exchange)),
        };

        let simulation_initial_balance = std::env::var("SIMULATION_INITIAL_BALANCE")
            .unwrap_or_else(|_| "10000".to_string())
            .parse()
            .unwrap_or_else(|_| rust_decimal::Decimal::from(10000));

        let simulation_price_volatility = std::env::var("SIMULATION_PRICE_VOLATILITY")
            .unwrap_or_else(|_| "0.02".to_string())
            .parse()
            .unwrap_or(0.02);

        let report_path = std::env::var("REPORT_PATH")
            .unwrap_or_else(|_| "portfolio_status.txt".to_string());

        let stop_loss_percent = std::env::var("STOP_LOSS_PERCENT")
            .unwrap_or_else(|_| "-5.0".to_string())
            .parse()
            .unwrap_or_else(|_| rust_decimal::Decimal::from(-5));

        let take_profit_percent = std::env::var("TAKE_PROFIT_PERCENT")
            .unwrap_or_else(|_| "10.0".to_string())
            .parse()
            .unwrap_or_else(|_| rust_decimal::Decimal::from(10));

        // Ollama settings
        let ollama_enabled = std::env::var("OLLAMA_ENABLED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(true);

        let ollama_url = std::env::var("OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());

        let ollama_model = std::env::var("OLLAMA_MODEL")
            .unwrap_or_else(|_| "mistral".to_string());

        Ok(Config {
            exchange,
            api_key: std::env::var("API_KEY").unwrap_or_default(),
            api_secret: std::env::var("API_SECRET").unwrap_or_default(),
            symbol: std::env::var("SYMBOL").unwrap_or_else(|_| "BTCUSDT".to_string()),
            base_url,
            ws_url,
            simulation_mode,
            simulation_initial_balance,
            simulation_price_volatility,
            report_path,
            stop_loss_percent,
            take_profit_percent,
            ollama_enabled,
            ollama_url,
            ollama_model,
        })
    }

    pub fn is_simulation(&self) -> bool {
        self.simulation_mode || self.exchange == "simulation"
    }
}
