use crate::config::Config;
use crate::models::{Balance, Order, OrderSide, OrderType};
use anyhow::Result;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

/// Simulated exchange for testing trading strategies without real money
pub struct SimulationExchange {
    config: Config,
    balances: Arc<Mutex<HashMap<String, Balance>>>,
    current_prices: Arc<Mutex<HashMap<String, Decimal>>>,
    orders: Arc<Mutex<Vec<Order>>>,
    order_id_counter: Arc<Mutex<i64>>,
    trade_history: Arc<Mutex<Vec<SimulatedTrade>>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SimulatedTrade {
    pub timestamp: i64,
    pub symbol: String,
    pub side: String,
    pub price: Decimal,
    pub quantity: Decimal,
    pub value: Decimal,
    pub pnl: Decimal,
}

impl SimulationExchange {
    pub async fn new(config: &Config) -> Result<Self> {
        let mut balances = HashMap::new();
        
        // Initialize with simulation balance (USDT)
        balances.insert(
            "USDT".to_string(),
            Balance {
                asset: "USDT".to_string(),
                free: config.simulation_initial_balance,
                locked: Decimal::ZERO,
            },
        );
        
        // Start with 0 BTC
        balances.insert(
            "BTC".to_string(),
            Balance {
                asset: "BTC".to_string(),
                free: Decimal::ZERO,
                locked: Decimal::ZERO,
            },
        );

        // Initialize with realistic starting prices
        let mut prices = HashMap::new();
        prices.insert("BTCUSDT".to_string(), dec!(42000.00));
        prices.insert("ETHUSDT".to_string(), dec!(2500.00));
        prices.insert("BNBUSDT".to_string(), dec!(300.00));

        info!("🎮 Simulation exchange initialized");
        info!("💰 Starting balance: {} USDT", config.simulation_initial_balance);

        Ok(Self {
            config: config.clone(),
            balances: Arc::new(Mutex::new(balances)),
            current_prices: Arc::new(Mutex::new(prices)),
            orders: Arc::new(Mutex::new(Vec::new())),
            order_id_counter: Arc::new(Mutex::new(1)),
            trade_history: Arc::new(Mutex::new(Vec::new())),
        })
    }

    fn timestamp() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }

    /// Simulate price movement with random walk
    fn simulate_price_movement(&self, current_price: Decimal) -> Decimal {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        // Random price change within volatility range
        let volatility = self.config.simulation_price_volatility;
        let change_percent = rng.gen_range(-volatility..volatility);
        let change = current_price * Decimal::try_from(change_percent).unwrap_or(Decimal::ZERO);
        
        let new_price = current_price + change;
        // Ensure price doesn't go negative
        if new_price > Decimal::ZERO { new_price } else { current_price }
    }

    pub async fn get_price(&self, symbol: &str) -> Result<Decimal> {
        let mut prices = self.current_prices.lock().unwrap();
        
        let current_price = prices.get(symbol).copied().unwrap_or(dec!(42000.00));
        let new_price = self.simulate_price_movement(current_price);
        prices.insert(symbol.to_string(), new_price);
        
        Ok(new_price)
    }

    pub async fn get_balance(&self) -> Result<HashMap<String, Balance>> {
        let balances = self.balances.lock().unwrap();
        Ok(balances.clone())
    }

    pub async fn place_order(
        &self,
        symbol: &str,
        side: OrderSide,
        _order_type: OrderType,
        quantity: Decimal,
        _price: Option<Decimal>,
    ) -> Result<Order> {
        let current_price = self.get_price(symbol).await?;
        let order_value = quantity * current_price;
        
        // Get base and quote assets from symbol (e.g., BTCUSDT -> BTC, USDT)
        let base_asset = symbol.replace("USDT", "");
        let quote_asset = "USDT".to_string();
        
        let mut balances = self.balances.lock().unwrap();
        
        match side {
            OrderSide::Buy => {
                // Check if we have enough USDT
                let usdt_balance = balances.get(&quote_asset).map(|b| b.free).unwrap_or(Decimal::ZERO);
                if usdt_balance < order_value {
                    return Err(anyhow::anyhow!(
                        "Insufficient balance: need {} USDT, have {}",
                        order_value,
                        usdt_balance
                    ));
                }
                
                // Deduct USDT
                if let Some(balance) = balances.get_mut(&quote_asset) {
                    balance.free -= order_value;
                }
                
                // Add base asset
                let base_balance = balances.entry(base_asset.clone()).or_insert(Balance {
                    asset: base_asset.clone(),
                    free: Decimal::ZERO,
                    locked: Decimal::ZERO,
                });
                base_balance.free += quantity;
                
                info!("🟢 SIMULATED BUY: {} {} @ {} = {} USDT", quantity, base_asset, current_price, order_value);
            }
            OrderSide::Sell => {
                // Check if we have enough base asset
                let base_balance_amount = balances.get(&base_asset).map(|b| b.free).unwrap_or(Decimal::ZERO);
                if base_balance_amount < quantity {
                    return Err(anyhow::anyhow!(
                        "Insufficient balance: need {} {}, have {}",
                        quantity,
                        base_asset,
                        base_balance_amount
                    ));
                }
                
                // Deduct base asset
                if let Some(balance) = balances.get_mut(&base_asset) {
                    balance.free -= quantity;
                }
                
                // Add USDT
                let usdt_balance = balances.entry(quote_asset.clone()).or_insert(Balance {
                    asset: quote_asset,
                    free: Decimal::ZERO,
                    locked: Decimal::ZERO,
                });
                usdt_balance.free += order_value;
                
                info!("🔴 SIMULATED SELL: {} {} @ {} = {} USDT", quantity, base_asset, current_price, order_value);
            }
        }
        
        // Create order
        let mut order_id = self.order_id_counter.lock().unwrap();
        let id = *order_id;
        *order_id += 1;
        
        let order = Order {
            symbol: symbol.to_string(),
            order_id: id,
            client_order_id: format!("sim_{}", id),
            price: current_price.to_string(),
            orig_qty: quantity.to_string(),
            executed_qty: quantity.to_string(),
            status: "FILLED".to_string(),
            side,
            order_type: _order_type,
        };
        
        // Store trade history
        let trade = SimulatedTrade {
            timestamp: Self::timestamp(),
            symbol: symbol.to_string(),
            side: format!("{:?}", side),
            price: current_price,
            quantity,
            value: order_value,
            pnl: Decimal::ZERO, // Would need entry price tracking for real PnL
        };
        self.trade_history.lock().unwrap().push(trade);
        self.orders.lock().unwrap().push(order.clone());
        
        Ok(order)
    }

    #[allow(dead_code)]
    pub async fn get_klines(
        &self,
        symbol: &str,
        _interval: &str,
        limit: u32,
    ) -> Result<Vec<crate::models::Kline>> {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        let current_price = self.get_price(symbol).await?;
        let mut klines = Vec::new();
        let mut price = current_price;
        
        // Generate synthetic historical data
        let now = Self::timestamp();
        for i in (0..limit).rev() {
            let volatility = self.config.simulation_price_volatility;
            let change = price * Decimal::try_from(rng.gen_range(-volatility..volatility)).unwrap_or(Decimal::ZERO);
            
            let open = price;
            let high = price + (price * Decimal::try_from(rng.gen_range(0.0..volatility)).unwrap_or(Decimal::ZERO));
            let low = price - (price * Decimal::try_from(rng.gen_range(0.0..volatility)).unwrap_or(Decimal::ZERO));
            let close = price + change;
            let volume = Decimal::try_from(rng.gen_range(100.0..1000.0)).unwrap_or(dec!(500));
            
            klines.push(crate::models::Kline {
                open_time: now - (i as i64 * 60000), // 1 minute intervals
                open,
                high,
                low,
                close,
                volume,
                close_time: now - ((i as i64 - 1) * 60000),
            });
            
            price = close;
        }
        
        Ok(klines)
    }

    /// Get summary of simulation performance
    #[allow(dead_code)]
    pub fn get_performance_summary(&self) -> SimulationSummary {
        let balances = self.balances.lock().unwrap();
        let trades = self.trade_history.lock().unwrap();
        
        let usdt_balance = balances.get("USDT").map(|b| b.free).unwrap_or(Decimal::ZERO);
        let btc_balance = balances.get("BTC").map(|b| b.free).unwrap_or(Decimal::ZERO);
        
        // Estimate total value in USDT
        let prices = self.current_prices.lock().unwrap();
        let btc_price = prices.get("BTCUSDT").copied().unwrap_or(dec!(42000));
        let total_value = usdt_balance + (btc_balance * btc_price);
        
        let pnl = total_value - self.config.simulation_initial_balance;
        let pnl_percent = if self.config.simulation_initial_balance > Decimal::ZERO {
            (pnl / self.config.simulation_initial_balance) * dec!(100)
        } else {
            Decimal::ZERO
        };

        SimulationSummary {
            initial_balance: self.config.simulation_initial_balance,
            current_balance: total_value,
            pnl,
            pnl_percent,
            total_trades: trades.len(),
            usdt_balance,
            btc_balance,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct SimulationSummary {
    pub initial_balance: Decimal,
    pub current_balance: Decimal,
    pub pnl: Decimal,
    pub pnl_percent: Decimal,
    pub total_trades: usize,
    pub usdt_balance: Decimal,
    pub btc_balance: Decimal,
}

impl std::fmt::Display for SimulationSummary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\n╔══════════════════════════════════════╗\n\
             ║     SIMULATION PERFORMANCE SUMMARY    ║\n\
             ╠══════════════════════════════════════╣\n\
             ║ Initial Balance:  {:>15} USDT ║\n\
             ║ Current Balance:  {:>15} USDT ║\n\
             ║ P&L:              {:>15} USDT ║\n\
             ║ P&L %:            {:>15.2}%    ║\n\
             ║ Total Trades:     {:>15}      ║\n\
             ╠══════════════════════════════════════╣\n\
             ║ USDT:             {:>15}      ║\n\
             ║ BTC:              {:>15}      ║\n\
             ╚══════════════════════════════════════╝",
            self.initial_balance.round_dp(2),
            self.current_balance.round_dp(2),
            self.pnl.round_dp(2),
            self.pnl_percent.round_dp(2),
            self.total_trades,
            self.usdt_balance.round_dp(2),
            self.btc_balance.round_dp(6),
        )
    }
}
