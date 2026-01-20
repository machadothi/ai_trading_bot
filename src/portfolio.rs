use crate::ai_advisor::{AiTradingTargets, TradingRecommendation};
use crate::models::{OrderSide, Signal};
use anyhow::Result;
use chrono::{DateTime, Local, Utc};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use std::collections::HashMap;
use std::fs;
use tracing::info;

/// Portfolio status that gets written to file on every update
#[derive(Debug, Clone)]
pub struct PortfolioStatus {
    // Timestamps
    pub last_updated: DateTime<Utc>,
    pub bot_started: DateTime<Utc>,
    
    // Trading parameters
    pub symbol: String,
    pub stop_loss_price: Option<Decimal>,
    pub stop_loss_percent: Decimal,
    pub take_profit_price: Option<Decimal>,
    pub take_profit_percent: Decimal,
    pub buy_target_price: Option<Decimal>,
    pub sell_target_price: Option<Decimal>,
    
    // Current market data
    pub current_price: Decimal,
    pub price_change_24h: Decimal,
    pub price_change_24h_percent: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    
    // Position info
    pub position_side: Option<OrderSide>,
    pub entry_price: Option<Decimal>,
    pub position_size: Decimal,
    pub position_value: Decimal,
    pub unrealized_pnl: Decimal,
    pub unrealized_pnl_percent: Decimal,
    
    // Balances
    pub balances: HashMap<String, Decimal>,
    pub total_portfolio_value: Decimal,
    
    // Performance stats
    pub realized_pnl: Decimal,
    pub total_trades: u32,
    pub winning_trades: u32,
    pub losing_trades: u32,
    pub win_rate: Decimal,
    pub largest_win: Decimal,
    pub largest_loss: Decimal,
    
    // Strategy signals
    pub current_signal: Signal,
    pub sma_short: Option<Decimal>,
    pub sma_long: Option<Decimal>,
    pub rsi: Option<Decimal>,
    
    // AI Advisor
    pub ai_enabled: bool,
    pub ai_recommendation: Option<TradingRecommendation>,
    pub ai_confidence: Option<Decimal>,
    pub ai_reasoning: Option<String>,
    
    // Support/Resistance levels
    pub support: Option<Decimal>,
    pub strong_support: Option<Decimal>,
    pub resistance: Option<Decimal>,
    pub strong_resistance: Option<Decimal>,
    pub pivot_point: Option<Decimal>,
    
    // Trade limiter
    pub trades_today: u32,
    pub max_trades_per_day: u32,
    pub can_trade: bool,
    pub next_trading_day: Option<String>,
    
    // Alerts
    pub active_alerts: Vec<String>,
    pub last_event: String,
    
    // Mode
    pub is_simulation: bool,
}

impl Default for PortfolioStatus {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            last_updated: now,
            bot_started: now,
            symbol: "BTCUSDT".to_string(),
            stop_loss_price: None,
            stop_loss_percent: dec!(-5.0),
            take_profit_price: None,
            take_profit_percent: dec!(10.0),
            buy_target_price: None,
            sell_target_price: None,
            current_price: Decimal::ZERO,
            price_change_24h: Decimal::ZERO,
            price_change_24h_percent: Decimal::ZERO,
            high_24h: Decimal::ZERO,
            low_24h: Decimal::ZERO,
            position_side: None,
            entry_price: None,
            position_size: Decimal::ZERO,
            position_value: Decimal::ZERO,
            unrealized_pnl: Decimal::ZERO,
            unrealized_pnl_percent: Decimal::ZERO,
            balances: HashMap::new(),
            total_portfolio_value: Decimal::ZERO,
            realized_pnl: Decimal::ZERO,
            total_trades: 0,
            winning_trades: 0,
            losing_trades: 0,
            win_rate: Decimal::ZERO,
            largest_win: Decimal::ZERO,
            largest_loss: Decimal::ZERO,
            current_signal: Signal::Hold,
            sma_short: None,
            sma_long: None,
            rsi: None,
            ai_enabled: false,
            ai_recommendation: None,
            ai_confidence: None,
            ai_reasoning: None,
            support: None,
            strong_support: None,
            resistance: None,
            strong_resistance: None,
            pivot_point: None,
            trades_today: 0,
            max_trades_per_day: 2,
            can_trade: true,
            next_trading_day: None,
            active_alerts: Vec::new(),
            last_event: "Bot started".to_string(),
            is_simulation: false,
        }
    }
}

impl PortfolioStatus {
    pub fn new(symbol: &str, is_simulation: bool) -> Self {
        Self {
            symbol: symbol.to_string(),
            is_simulation,
            ..Default::default()
        }
    }

    /// Update stop-loss and take-profit prices based on entry price
    pub fn update_targets(&mut self) {
        if let Some(entry) = self.entry_price {
            self.stop_loss_price = Some(entry * (dec!(1) + self.stop_loss_percent / dec!(100)));
            self.take_profit_price = Some(entry * (dec!(1) + self.take_profit_percent / dec!(100)));
        }
    }

    /// Calculate unrealized P&L
    pub fn update_unrealized_pnl(&mut self) {
        if let Some(entry) = self.entry_price {
            if self.position_size > Decimal::ZERO {
                self.position_value = self.position_size * self.current_price;
                let entry_value = self.position_size * entry;
                self.unrealized_pnl = self.position_value - entry_value;
                
                if entry_value > Decimal::ZERO {
                    self.unrealized_pnl_percent = (self.unrealized_pnl / entry_value) * dec!(100);
                }
            }
        }
    }

    /// Check if any price targets are hit
    pub fn check_targets(&self) -> Option<String> {
        if let Some(stop_loss) = self.stop_loss_price {
            if self.current_price <= stop_loss {
                return Some(format!("🔴 STOP-LOSS HIT at {}", self.current_price));
            }
        }
        
        if let Some(take_profit) = self.take_profit_price {
            if self.current_price >= take_profit {
                return Some(format!("🟢 TAKE-PROFIT HIT at {}", self.current_price));
            }
        }
        
        if let Some(buy_target) = self.buy_target_price {
            if self.current_price <= buy_target {
                return Some(format!("🔵 BUY TARGET HIT at {}", self.current_price));
            }
        }
        
        if let Some(sell_target) = self.sell_target_price {
            if self.current_price >= sell_target {
                return Some(format!("🟠 SELL TARGET HIT at {}", self.current_price));
            }
        }
        
        None
    }

    /// Update win rate calculation
    pub fn update_stats(&mut self) {
        if self.total_trades > 0 {
            self.win_rate = Decimal::from(self.winning_trades) / Decimal::from(self.total_trades) * dec!(100);
        }
    }
}

pub struct PortfolioReporter {
    status: PortfolioStatus,
    report_path: String,
}

impl PortfolioReporter {
    pub fn new(symbol: &str, is_simulation: bool, report_path: &str) -> Self {
        Self {
            status: PortfolioStatus::new(symbol, is_simulation),
            report_path: report_path.to_string(),
        }
    }

    pub fn status_mut(&mut self) -> &mut PortfolioStatus {
        &mut self.status
    }

    pub fn status(&self) -> &PortfolioStatus {
        &self.status
    }

    /// Update price and check for events
    pub fn update_price(&mut self, price: Decimal) -> Option<String> {
        self.status.current_price = price;
        self.status.update_unrealized_pnl();
        self.status.last_updated = Utc::now();
        
        // Check if any targets were hit
        if let Some(event) = self.status.check_targets() {
            self.status.last_event = event.clone();
            self.status.active_alerts.push(event.clone());
            self.write_report().ok();
            return Some(event);
        }
        
        None
    }

    /// Record a trade execution
    pub fn record_trade(&mut self, side: OrderSide, price: Decimal, quantity: Decimal, pnl: Option<Decimal>) {
        self.status.total_trades += 1;
        
        if let Some(profit) = pnl {
            self.status.realized_pnl += profit;
            
            if profit > Decimal::ZERO {
                self.status.winning_trades += 1;
                if profit > self.status.largest_win {
                    self.status.largest_win = profit;
                }
            } else {
                self.status.losing_trades += 1;
                if profit < self.status.largest_loss {
                    self.status.largest_loss = profit;
                }
            }
        }
        
        match side {
            OrderSide::Buy => {
                self.status.entry_price = Some(price);
                self.status.position_size = quantity;
                self.status.position_side = Some(OrderSide::Buy);
                self.status.update_targets();
                self.status.last_event = format!("🟢 BUY executed: {} @ {}", quantity, price);
            }
            OrderSide::Sell => {
                self.status.entry_price = None;
                self.status.position_size = Decimal::ZERO;
                self.status.position_side = None;
                self.status.stop_loss_price = None;
                self.status.take_profit_price = None;
                self.status.last_event = format!("🔴 SELL executed: {} @ {}", quantity, price);
            }
        }
        
        self.status.update_stats();
        self.status.last_updated = Utc::now();
        self.write_report().ok();
    }

    /// Update balances
    pub fn update_balances(&mut self, balances: HashMap<String, Decimal>) {
        self.status.balances = balances;
        self.status.total_portfolio_value = self.status.balances.values().sum();
        self.status.last_updated = Utc::now();
    }

    /// Update strategy signals
    pub fn update_signals(&mut self, signal: Signal, sma_short: Option<Decimal>, sma_long: Option<Decimal>, rsi: Option<Decimal>) {
        let old_signal = self.status.current_signal;
        self.status.current_signal = signal;
        self.status.sma_short = sma_short;
        self.status.sma_long = sma_long;
        self.status.rsi = rsi;
        
        // If signal changed, write report
        if old_signal != signal {
            self.status.last_event = format!("📊 Signal changed: {:?} -> {:?}", old_signal, signal);
            self.status.last_updated = Utc::now();
            self.write_report().ok();
        }
    }

    /// Update AI-calculated trading targets
    pub fn update_ai_targets(&mut self, targets: &AiTradingTargets) {
        self.status.ai_enabled = true;
        self.status.stop_loss_price = Some(targets.stop_loss_price);
        self.status.take_profit_price = Some(targets.take_profit_price);
        self.status.buy_target_price = targets.buy_target_price;
        self.status.sell_target_price = targets.sell_target_price;
        self.status.ai_recommendation = Some(targets.recommendation.clone());
        self.status.ai_confidence = Some(targets.confidence);
        self.status.ai_reasoning = Some(targets.reasoning.clone());
        
        // Update support/resistance levels
        self.status.support = targets.support;
        self.status.strong_support = targets.strong_support;
        self.status.resistance = targets.resistance;
        self.status.strong_resistance = targets.strong_resistance;
        self.status.pivot_point = targets.pivot_point;
        
        self.status.last_event = format!("🤖 AI targets updated: {} ({}% confidence)", 
            targets.recommendation, targets.confidence.round_dp(0));
        self.status.last_updated = Utc::now();
        self.write_report().ok();
    }

    /// Update trade limiter status
    pub fn update_trade_limits(&mut self, trades_today: u32, can_trade: bool, next_day: Option<String>) {
        self.status.trades_today = trades_today;
        self.status.can_trade = can_trade;
        self.status.next_trading_day = next_day;
        self.status.last_updated = Utc::now();
    }

    /// Force write report
    pub fn force_write(&mut self) -> Result<()> {
        self.status.last_updated = Utc::now();
        self.write_report()
    }

    /// Write the portfolio report to file
    pub fn write_report(&self) -> Result<()> {
        let s = &self.status;
        let local_time: DateTime<Local> = s.last_updated.into();
        let started_local: DateTime<Local> = s.bot_started.into();
        
        let mode_banner = if s.is_simulation {
            "║           🎮 SIMULATION MODE 🎮           ║"
        } else {
            "║             💰 LIVE TRADING 💰             ║"
        };

        let position_status = match &s.position_side {
            Some(OrderSide::Buy) => "LONG",
            Some(OrderSide::Sell) => "SHORT",
            None => "NO POSITION",
        };

        let signal_emoji = match s.current_signal {
            Signal::Buy => "🟢 BUY",
            Signal::Sell => "🔴 SELL",
            Signal::Hold => "⚪ HOLD",
        };

        // Format AI section
        let rec_emoji = match &s.ai_recommendation {
            Some(TradingRecommendation::StrongBuy) => "🟢🟢 STRONG BUY",
            Some(TradingRecommendation::Buy) => "🟢 BUY",
            Some(TradingRecommendation::Hold) => "⚪ HOLD",
            Some(TradingRecommendation::Sell) => "🔴 SELL",
            Some(TradingRecommendation::StrongSell) => "🔴🔴 STRONG SELL",
            None => "N/A",
        };
        let confidence = s.ai_confidence.map(|c| format!("{}%", c.round_dp(0))).unwrap_or_else(|| "N/A".to_string());
        let reasoning = s.ai_reasoning.as_deref().unwrap_or("No analysis available");
        
        // Format support/resistance section
        let sr_section = if s.support.is_some() || s.resistance.is_some() {
            format!(r#"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📐 SUPPORT & RESISTANCE
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Strong Resistance: {strong_res}
  Resistance (R1):   {res}
  ─── Pivot Point:   {pivot} ───
  Support (S1):      {sup}
  Strong Support:    {strong_sup}
"#,
                strong_res = s.strong_resistance.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "Not calculated".to_string()),
                res = s.resistance.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "Not calculated".to_string()),
                pivot = s.pivot_point.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "N/A".to_string()),
                sup = s.support.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "Not calculated".to_string()),
                strong_sup = s.strong_support.map(|p| format!("${:.2}", p)).unwrap_or_else(|| "Not calculated".to_string()),
            )
        } else {
            String::new()
        };
        
        // Format trade limiter section
        let trade_limit_section = format!(r#"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 DAILY TRADE LIMITS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Trades Today:      {trades}/{max}
  Can Trade:         {can_trade}
  {next_day}
"#,
            trades = s.trades_today,
            max = s.max_trades_per_day,
            can_trade = if s.can_trade { "✅ Yes" } else { "❌ No (limit reached)" },
            next_day = s.next_trading_day.as_ref().map(|d| format!("Next Trading Day: {}", d)).unwrap_or_default(),
        );

        let ai_section = if s.ai_enabled {
            format!(r#"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🧠 AI ADVISOR (Ollama)
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Recommendation:    {rec}
  Confidence:        {conf}
  Analysis:          {reason}
"#, rec = rec_emoji, conf = confidence, reason = reasoning)
        } else {
            format!(r#"
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🧠 AI ADVISOR
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Status:            ⚠️  Not connected (using fallback)
  To enable:         Install Ollama and run: ollama pull mistral
"#)
        };

        let report = format!(r#"
╔════════════════════════════════════════════╗
{mode_banner}
╠════════════════════════════════════════════╣
║  CRYPTO TRADING BOT - PORTFOLIO STATUS     ║
╚════════════════════════════════════════════╝

📅 Last Updated: {last_updated}
🚀 Bot Started:  {started}
⏱️  Uptime:       {uptime}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📊 MARKET DATA - {symbol}
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Current Price:     ${current_price}
  24h Change:        ${change_24h} ({change_percent}%)
  24h High:          ${high_24h}
  24h Low:           ${low_24h}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎯 TRADING TARGETS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Stop-Loss:         {stop_loss} ({stop_loss_pct}%)
  Take-Profit:       {take_profit} ({take_profit_pct}%)
  Buy Target:        {buy_target}
  Sell Target:       {sell_target}
{sr_section}{ai_section}{trade_limit_section}
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📈 CURRENT POSITION
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Status:            {position_status}
  Entry Price:       {entry_price}
  Position Size:     {position_size}
  Position Value:    ${position_value}
  Unrealized P&L:    ${unrealized_pnl} ({unrealized_pnl_pct}%)

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
💰 BALANCES
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
{balances}
  ─────────────────────────────────
  Total Portfolio:   ${total_value}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📉 PERFORMANCE STATISTICS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Realized P&L:      ${realized_pnl}
  Total Trades:      {total_trades}
  Winning Trades:    {winning_trades}
  Losing Trades:     {losing_trades}
  Win Rate:          {win_rate}%
  Largest Win:       ${largest_win}
  Largest Loss:      ${largest_loss}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🤖 STRATEGY SIGNALS
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Current Signal:    {signal}
  SMA Short:         {sma_short}
  SMA Long:          {sma_long}
  RSI (14):          {rsi}

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🔔 LAST EVENT
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  {last_event}

{alerts_section}
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
"#,
            mode_banner = mode_banner,
            last_updated = local_time.format("%Y-%m-%d %H:%M:%S"),
            started = started_local.format("%Y-%m-%d %H:%M:%S"),
            uptime = format_duration(s.last_updated.signed_duration_since(s.bot_started)),
            symbol = s.symbol,
            current_price = s.current_price.round_dp(2),
            change_24h = s.price_change_24h.round_dp(2),
            change_percent = s.price_change_24h_percent.round_dp(2),
            high_24h = s.high_24h.round_dp(2),
            low_24h = s.low_24h.round_dp(2),
            stop_loss = s.stop_loss_price.map(|p| format!("${}", p.round_dp(2))).unwrap_or_else(|| "Not set".to_string()),
            stop_loss_pct = s.stop_loss_percent,
            take_profit = s.take_profit_price.map(|p| format!("${}", p.round_dp(2))).unwrap_or_else(|| "Not set".to_string()),
            take_profit_pct = s.take_profit_percent,
            buy_target = s.buy_target_price.map(|p| format!("${}", p.round_dp(2))).unwrap_or_else(|| "Not set".to_string()),
            sell_target = s.sell_target_price.map(|p| format!("${}", p.round_dp(2))).unwrap_or_else(|| "Not set".to_string()),
            sr_section = sr_section,
            ai_section = ai_section,
            trade_limit_section = trade_limit_section,
            position_status = position_status,
            entry_price = s.entry_price.map(|p| format!("${}", p.round_dp(2))).unwrap_or_else(|| "N/A".to_string()),
            position_size = s.position_size.round_dp(6),
            position_value = s.position_value.round_dp(2),
            unrealized_pnl = s.unrealized_pnl.round_dp(2),
            unrealized_pnl_pct = s.unrealized_pnl_percent.round_dp(2),
            balances = format_balances(&s.balances),
            total_value = s.total_portfolio_value.round_dp(2),
            realized_pnl = s.realized_pnl.round_dp(2),
            total_trades = s.total_trades,
            winning_trades = s.winning_trades,
            losing_trades = s.losing_trades,
            win_rate = s.win_rate.round_dp(1),
            largest_win = s.largest_win.round_dp(2),
            largest_loss = s.largest_loss.round_dp(2),
            signal = signal_emoji,
            sma_short = s.sma_short.map(|v| format!("{}", v.round_dp(2))).unwrap_or_else(|| "N/A".to_string()),
            sma_long = s.sma_long.map(|v| format!("{}", v.round_dp(2))).unwrap_or_else(|| "N/A".to_string()),
            rsi = s.rsi.map(|v| format!("{}", v.round_dp(2))).unwrap_or_else(|| "N/A".to_string()),
            last_event = s.last_event,
            alerts_section = format_alerts(&s.active_alerts),
        );

        // Write to file (overwrites completely)
        fs::write(&self.report_path, report.trim())?;
        info!("📄 Portfolio report written to {}", self.report_path);
        
        Ok(())
    }
}

fn format_balances(balances: &HashMap<String, Decimal>) -> String {
    if balances.is_empty() {
        return "  No balances".to_string();
    }
    
    let mut result = Vec::new();
    for (asset, amount) in balances {
        if *amount > Decimal::ZERO {
            result.push(format!("  {:<18} {}", format!("{}:", asset), amount.round_dp(6)));
        }
    }
    
    if result.is_empty() {
        "  No balances".to_string()
    } else {
        result.join("\n")
    }
}

fn format_alerts(alerts: &[String]) -> String {
    if alerts.is_empty() {
        return String::new();
    }
    
    let mut result = String::from("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n⚠️  RECENT ALERTS\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    
    for alert in alerts.iter().rev().take(5) {
        result.push_str(&format!("  • {}\n", alert));
    }
    
    result
}

fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}
