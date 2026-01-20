use anyhow::Result;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::fs;
use tracing::{info, warn};

/// Trade record for tracking daily limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeRecord {
    pub timestamp: DateTime<Utc>,
    pub symbol: String,
    pub side: String, // "BUY" or "SELL"
    pub price: Decimal,
    pub quantity: Decimal,
    pub is_first_trade: bool,
}

/// Daily trading state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyTradingState {
    pub date: String, // YYYY-MM-DD format
    pub trades_today: Vec<TradeRecord>,
    pub first_trade_executed: bool,
    pub second_trade_executed: bool,
    pub daily_pnl: Decimal,
}

/// Trade limiter - enforces max 2 trades per day rule
pub struct TradeLimiter {
    state_file: String,
    current_state: DailyTradingState,
    max_trades_per_day: u32,
}

impl TradeLimiter {
    pub fn new(state_file: &str) -> Self {
        let mut limiter = Self {
            state_file: state_file.to_string(),
            current_state: DailyTradingState::new_for_today(),
            max_trades_per_day: 2,
        };
        limiter.load_state();
        limiter
    }

    /// Load state from file, reset if it's a new day
    fn load_state(&mut self) {
        let today = Self::today_string();
        
        if let Ok(content) = fs::read_to_string(&self.state_file) {
            if let Ok(state) = serde_json::from_str::<DailyTradingState>(&content) {
                if state.date == today {
                    self.current_state = state;
                    info!("Loaded trading state for today: {} trades executed", 
                          self.current_state.trades_today.len());
                    return;
                } else {
                    info!("New trading day detected, resetting state");
                }
            }
        }
        
        // Start fresh for today
        self.current_state = DailyTradingState::new_for_today();
        self.save_state();
    }

    /// Save state to file
    fn save_state(&self) {
        if let Ok(json) = serde_json::to_string_pretty(&self.current_state) {
            if let Err(e) = fs::write(&self.state_file, json) {
                warn!("Failed to save trade limiter state: {}", e);
            }
        }
    }

    fn today_string() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    /// Check if trading is allowed
    pub fn can_trade(&self) -> TradePermission {
        let today = Self::today_string();
        
        // Reset if it's a new day
        if self.current_state.date != today {
            return TradePermission::Allowed {
                is_first_trade: true,
                trades_remaining: 2,
            };
        }

        let trades_count = self.current_state.trades_today.len() as u32;

        if trades_count >= self.max_trades_per_day {
            TradePermission::DailyLimitReached {
                trades_executed: trades_count,
                next_trading_day: self.next_trading_day(),
            }
        } else if trades_count == 0 {
            TradePermission::Allowed {
                is_first_trade: true,
                trades_remaining: 2,
            }
        } else {
            // Second trade is only allowed if first trade was executed
            TradePermission::Allowed {
                is_first_trade: false,
                trades_remaining: 1,
            }
        }
    }

    /// Record a trade
    pub fn record_trade(
        &mut self,
        symbol: &str,
        side: &str,
        price: Decimal,
        quantity: Decimal,
    ) -> Result<()> {
        let today = Self::today_string();
        
        // Reset if new day
        if self.current_state.date != today {
            self.current_state = DailyTradingState::new_for_today();
        }

        let is_first = self.current_state.trades_today.is_empty();
        
        let record = TradeRecord {
            timestamp: Utc::now(),
            symbol: symbol.to_string(),
            side: side.to_string(),
            price,
            quantity,
            is_first_trade: is_first,
        };

        self.current_state.trades_today.push(record);
        
        if is_first {
            self.current_state.first_trade_executed = true;
        } else {
            self.current_state.second_trade_executed = true;
        }

        self.save_state();
        
        info!(
            "Trade recorded: {} {} {} @ {}. Trades today: {}/{}",
            side, quantity, symbol, price,
            self.current_state.trades_today.len(),
            self.max_trades_per_day
        );

        Ok(())
    }

    /// Get current trading status
    pub fn get_status(&self) -> TradingStatus {
        let today = Self::today_string();
        
        if self.current_state.date != today {
            return TradingStatus {
                date: today,
                trades_executed: 0,
                trades_remaining: 2,
                first_trade: None,
                second_trade: None,
                daily_pnl: Decimal::ZERO,
                can_trade: true,
            };
        }

        let trades_count = self.current_state.trades_today.len();
        
        TradingStatus {
            date: self.current_state.date.clone(),
            trades_executed: trades_count as u32,
            trades_remaining: (self.max_trades_per_day as usize).saturating_sub(trades_count) as u32,
            first_trade: self.current_state.trades_today.first().cloned(),
            second_trade: self.current_state.trades_today.get(1).cloned(),
            daily_pnl: self.current_state.daily_pnl,
            can_trade: trades_count < self.max_trades_per_day as usize,
        }
    }

    /// Update daily P&L
    pub fn update_pnl(&mut self, pnl: Decimal) {
        self.current_state.daily_pnl = pnl;
        self.save_state();
    }

    fn next_trading_day(&self) -> String {
        let tomorrow = Utc::now() + chrono::Duration::days(1);
        tomorrow.format("%Y-%m-%d").to_string()
    }

    /// Get trades for today
    #[allow(dead_code)]
    pub fn get_todays_trades(&self) -> &[TradeRecord] {
        &self.current_state.trades_today
    }
}

impl DailyTradingState {
    fn new_for_today() -> Self {
        Self {
            date: Utc::now().format("%Y-%m-%d").to_string(),
            trades_today: Vec::new(),
            first_trade_executed: false,
            second_trade_executed: false,
            daily_pnl: Decimal::ZERO,
        }
    }
}

/// Result of checking if trading is allowed
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum TradePermission {
    Allowed {
        is_first_trade: bool,
        trades_remaining: u32,
    },
    DailyLimitReached {
        trades_executed: u32,
        next_trading_day: String,
    },
}

impl TradePermission {
    #[allow(dead_code)]
    pub fn is_allowed(&self) -> bool {
        matches!(self, TradePermission::Allowed { .. })
    }
}

/// Current trading status summary
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct TradingStatus {
    pub date: String,
    pub trades_executed: u32,
    pub trades_remaining: u32,
    pub first_trade: Option<TradeRecord>,
    pub second_trade: Option<TradeRecord>,
    pub daily_pnl: Decimal,
    pub can_trade: bool,
}

impl std::fmt::Display for TradingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Date: {}, Trades: {}/2, P&L: ${:.2}, Can Trade: {}",
            self.date,
            self.trades_executed,
            self.daily_pnl,
            if self.can_trade { "Yes" } else { "No" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_limiter() {
        let limiter = TradeLimiter::new("/tmp/test_trade_state.json");
        assert!(limiter.can_trade().is_allowed());
    }

    #[test]
    fn test_trade_permission() {
        let permission = TradePermission::Allowed {
            is_first_trade: true,
            trades_remaining: 2,
        };
        assert!(permission.is_allowed());
    }
}
