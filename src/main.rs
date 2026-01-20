mod ai_advisor;
mod coingecko;
mod config;
mod exchange;
mod models;
mod portfolio;
mod simulation;
mod strategy;
mod trade_limiter;

use ai_advisor::{AiTradingTargets, FallbackTargetCalculator, MarketContext, OllamaClient};
use anyhow::Result;
use coingecko::CoinGeckoClient;
use portfolio::PortfolioReporter;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use trade_limiter::{TradeLimiter, TradePermission};
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::time::Duration;

// How often to check prices (in seconds)
const PRICE_CHECK_INTERVAL_SECS: u64 = 30;
// How often to recalculate targets with AI (in seconds)
const AI_RECALC_INTERVAL_SECS: u64 = 300; // 5 minutes

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load environment variables
    dotenv::dotenv().ok();

    info!("🚀 Crypto Trading Bot starting...");

    // Load configuration
    let config = config::Config::from_env()?;
    
    if config.is_simulation() {
        info!("🎮 Running in SIMULATION MODE - no real trades will be executed");
        run_simulation_loop(config).await
    } else {
        info!("💰 Running in LIVE MODE on exchange: {}", config.exchange);
        warn!("⚠️  Real money is at risk!");
        run_live_loop(config).await
    }
}

/// Continuous monitoring loop for simulation mode
async fn run_simulation_loop(config: config::Config) -> Result<()> {
    let exchange = simulation::SimulationExchange::new(&config).await?;
    info!("✅ Simulation exchange initialized");

    // Initialize components
    let mut reporter = PortfolioReporter::new(&config.symbol, true, &config.report_path);
    let coingecko = CoinGeckoClient::new();
    let mut trade_limiter = TradeLimiter::new(&format!(
        "{}/trade_state.json",
        std::env::current_dir()?.display()
    ));

    // Set trading parameters
    {
        let status = reporter.status_mut();
        status.stop_loss_percent = config.stop_loss_percent;
        status.take_profit_percent = config.take_profit_percent;
    }

    // Get initial balance
    let balance = exchange.get_balance().await?;
    let balance_map: std::collections::HashMap<String, Decimal> = balance
        .iter()
        .map(|(k, v)| (k.clone(), v.free))
        .collect();
    reporter.update_balances(balance_map);
    info!("💰 Starting balance: {:?}", balance);

    // Track state
    let mut last_ai_update = std::time::Instant::now();
    let mut current_targets: Option<AiTradingTargets> = None;
    let mut in_position = false;
    let mut position_qty = dec!(0);
    let mut loop_count: u64 = 0;

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🔄 Starting CONTINUOUS monitoring loop...");
    info!("   Price check interval: {}s", PRICE_CHECK_INTERVAL_SECS);
    info!("   AI recalculation interval: {}s", AI_RECALC_INTERVAL_SECS);
    info!("   Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    loop {
        loop_count += 1;
        info!("");
        info!("━━━ Monitoring cycle #{} ━━━", loop_count);

        // Fetch real market data from CoinGecko
        let market_data = match coingecko.fetch_market_data(&config.symbol).await {
            Ok(data) => {
                info!("✅ CoinGecko: {} @ ${:.2}", data.symbol, data.current_price);
                Some(data)
            }
            Err(e) => {
                warn!("⚠️ CoinGecko fetch failed: {}", e);
                None
            }
        };

        // Get current price (from CoinGecko or simulated)
        let current_price = if let Some(ref data) = market_data {
            data.current_price
        } else {
            exchange.get_price(&config.symbol).await.unwrap_or(dec!(0))
        };

        if current_price == dec!(0) {
            warn!("❌ Could not get current price, skipping cycle");
            tokio::time::sleep(Duration::from_secs(PRICE_CHECK_INTERVAL_SECS)).await;
            continue;
        }

        // Update reporter with price
        if let Some(event) = reporter.update_price(current_price) {
            info!("🔔 ALERT: {}", event);
        }

        // Calculate support/resistance if we have market data
        let (sma_short, sma_long, rsi, high_24h, low_24h, change_24h) = if let Some(ref data) = market_data {
            let closes: Vec<Decimal> = data.hourly_data_24h.iter().map(|d| d.close).collect();
            let sma_s = strategy::SmaCrossover::calculate_sma(&closes, 10);
            let sma_l = strategy::SmaCrossover::calculate_sma(&closes, 20);
            let rsi_val = strategy::RsiStrategy::calculate_rsi(&closes, 14);
            (sma_s, sma_l, rsi_val, data.high_24h, data.low_24h, data.price_change_24h_percent)
        } else {
            (None, None, None, current_price * dec!(1.02), current_price * dec!(0.98), dec!(0))
        };

        // Build market context
        let market_context = MarketContext {
            symbol: config.symbol.clone(),
            current_price,
            high_24h,
            low_24h,
            price_change_24h_percent: change_24h,
            sma_short,
            sma_long,
            rsi,
            volume_24h: market_data.as_ref().map(|d| d.total_volume),
            position_entry_price: reporter.status().entry_price,
            account_balance: reporter.status().total_portfolio_value,
            hourly_data_summary: market_data.as_ref().map(|d| coingecko.format_for_ai(d)),
            high_12h: market_data.as_ref().and_then(|d| d.hourly_data_12h.iter().map(|h| h.high).max()),
            low_12h: market_data.as_ref().and_then(|d| d.hourly_data_12h.iter().map(|h| h.low).min()),
            high_48h: market_data.as_ref().and_then(|d| d.hourly_data_48h.iter().map(|h| h.high).max()),
            low_48h: market_data.as_ref().and_then(|d| d.hourly_data_48h.iter().map(|h| h.low).min()),
        };

        // Update reporter market data
        {
            let status = reporter.status_mut();
            status.current_price = current_price;
            status.high_24h = high_24h;
            status.low_24h = low_24h;
            status.price_change_24h_percent = change_24h;
        }

        // Update signals
        let signal = if let (Some(short), Some(long)) = (sma_short, sma_long) {
            if short > long { models::Signal::Buy } else { models::Signal::Sell }
        } else {
            models::Signal::Hold
        };
        reporter.update_signals(signal, sma_short, sma_long, rsi);

        // Recalculate targets periodically or if we don't have any
        let should_recalc = current_targets.is_none() 
            || last_ai_update.elapsed().as_secs() >= AI_RECALC_INTERVAL_SECS;

        if should_recalc {
            info!("🔄 Recalculating trading targets...");
            
            // Always calculate fallback first
            let fallback = FallbackTargetCalculator::calculate_targets(&market_context);
            current_targets = Some(fallback.clone());
            reporter.update_ai_targets(&fallback);
            info!("📊 Fallback: {} @ {}% confidence", fallback.recommendation, fallback.confidence.round_dp(0));

            // Try AI if enabled (non-blocking with timeout)
            if config.ollama_enabled {
                match OllamaClient::new(Some(&config.ollama_url), Some(&config.ollama_model)) {
                    Ok(ollama) => {
                        if ollama.health_check().await.unwrap_or(false) {
                            info!("🤖 Requesting AI analysis (timeout: 120s)...");
                            
                            match tokio::time::timeout(
                                Duration::from_secs(120),
                                ollama.calculate_targets(&market_context)
                            ).await {
                                Ok(Ok(targets)) => {
                                    info!("🧠 AI: {} @ {}% confidence", 
                                        targets.recommendation, targets.confidence.round_dp(0));
                                    current_targets = Some(targets.clone());
                                    reporter.update_ai_targets(&targets);
                                }
                                Ok(Err(e)) => warn!("⚠️ AI analysis failed: {}", e),
                                Err(_) => warn!("⚠️ AI analysis timed out"),
                            }
                        }
                    }
                    Err(e) => warn!("⚠️ Ollama client error: {}", e),
                }
            }

            last_ai_update = std::time::Instant::now();
        }

        // Check trade limits
        let trade_status = trade_limiter.get_status();
        reporter.update_trade_limits(
            trade_status.trades_executed,
            trade_status.can_trade,
            if trade_status.can_trade { None } else { Some(trade_status.date.clone()) },
        );

        // Trading logic - check if targets are hit
        if let Some(ref targets) = current_targets {
            let can_trade = matches!(trade_limiter.can_trade(), TradePermission::Allowed { .. });

            if in_position {
                // We have a position - check for exit signals
                let entry = reporter.status().entry_price.unwrap_or(current_price);
                
                // Check stop-loss
                if current_price <= targets.stop_loss_price {
                    info!("🔴 STOP-LOSS TRIGGERED at ${:.2}!", current_price);
                    if can_trade {
                        let pnl = (current_price - entry) * position_qty;
                        execute_sell(&exchange, &config.symbol, position_qty, current_price, pnl, 
                                    &mut reporter, &mut trade_limiter).await?;
                        in_position = false;
                        position_qty = dec!(0);
                    } else {
                        warn!("⚠️ Cannot execute - daily trade limit reached");
                    }
                }
                // Check take-profit
                else if current_price >= targets.take_profit_price {
                    info!("🟢 TAKE-PROFIT TRIGGERED at ${:.2}!", current_price);
                    if can_trade {
                        let pnl = (current_price - entry) * position_qty;
                        execute_sell(&exchange, &config.symbol, position_qty, current_price, pnl,
                                    &mut reporter, &mut trade_limiter).await?;
                        in_position = false;
                        position_qty = dec!(0);
                    } else {
                        warn!("⚠️ Cannot execute - daily trade limit reached");
                    }
                }
                // Check sell target
                else if let Some(sell_target) = targets.sell_target_price {
                    if current_price >= sell_target {
                        info!("💜 SELL TARGET reached at ${:.2}!", current_price);
                        if can_trade {
                            let pnl = (current_price - entry) * position_qty;
                            execute_sell(&exchange, &config.symbol, position_qty, current_price, pnl,
                                        &mut reporter, &mut trade_limiter).await?;
                            in_position = false;
                            position_qty = dec!(0);
                        }
                    }
                }
            } else {
                // No position - check for entry signals
                if let Some(buy_target) = targets.buy_target_price {
                    if current_price <= buy_target && can_trade {
                        info!("💚 BUY TARGET reached at ${:.2}!", current_price);
                        
                        // Calculate position size (use 10% of balance for simulation)
                        let balance = reporter.status().balances.get("USDT").copied().unwrap_or(dec!(0));
                        let trade_amount = balance * dec!(0.10); // 10% of balance
                        let qty = trade_amount / current_price;
                        
                        if qty > dec!(0) {
                            execute_buy(&exchange, &config.symbol, qty, current_price,
                                       &mut reporter, &mut trade_limiter).await?;
                            in_position = true;
                            position_qty = qty;
                        }
                    }
                }
            }
        }

        // Update balances
        let balance = exchange.get_balance().await?;
        let balance_map: std::collections::HashMap<String, Decimal> = balance
            .iter()
            .map(|(k, v)| (k.clone(), v.free))
            .collect();
        reporter.update_balances(balance_map);

        // Write report
        reporter.force_write()?;

        // Log current state summary
        if let Some(ref targets) = current_targets {
            info!("📍 Price: ${:.2} | SL: ${:.2} | TP: ${:.2}", 
                current_price, targets.stop_loss_price, targets.take_profit_price);
            if let (Some(buy), Some(sell)) = (targets.buy_target_price, targets.sell_target_price) {
                info!("   Buy Target: ${:.2} | Sell Target: ${:.2}", buy, sell);
            }
            info!("   Position: {} | Trades today: {}/2", 
                if in_position { "LONG" } else { "NONE" }, trade_status.trades_executed);
        }

        // Wait before next cycle
        info!("💤 Sleeping {}s until next check...", PRICE_CHECK_INTERVAL_SECS);
        tokio::time::sleep(Duration::from_secs(PRICE_CHECK_INTERVAL_SECS)).await;
    }
}

async fn execute_buy(
    exchange: &simulation::SimulationExchange,
    symbol: &str,
    qty: Decimal,
    price: Decimal,
    reporter: &mut PortfolioReporter,
    trade_limiter: &mut TradeLimiter,
) -> Result<()> {
    let _order = exchange.place_order(
        symbol,
        models::OrderSide::Buy,
        models::OrderType::Market,
        qty,
        None,
    ).await?;
    
    trade_limiter.record_trade(symbol, "BUY", price, qty)?;
    reporter.record_trade(models::OrderSide::Buy, price, qty, None);
    
    info!("✅ BUY executed: {} @ ${:.2}", qty.round_dp(6), price.round_dp(2));
    Ok(())
}

async fn execute_sell(
    exchange: &simulation::SimulationExchange,
    symbol: &str,
    qty: Decimal,
    price: Decimal,
    pnl: Decimal,
    reporter: &mut PortfolioReporter,
    trade_limiter: &mut TradeLimiter,
) -> Result<()> {
    let _order = exchange.place_order(
        symbol,
        models::OrderSide::Sell,
        models::OrderType::Market,
        qty,
        None,
    ).await?;
    
    trade_limiter.record_trade(symbol, "SELL", price, qty)?;
    trade_limiter.update_pnl(pnl);
    reporter.record_trade(models::OrderSide::Sell, price, qty, Some(pnl));
    
    let pnl_emoji = if pnl >= dec!(0) { "🟢" } else { "🔴" };
    info!("{} SELL executed: {} @ ${:.2} | P&L: ${:.2}", 
        pnl_emoji, qty.round_dp(6), price.round_dp(2), pnl.round_dp(2));
    Ok(())
}

/// Continuous monitoring loop for live trading
async fn run_live_loop(config: config::Config) -> Result<()> {
    let exchange = exchange::ExchangeClient::new(&config).await?;
    info!("✅ Connected to exchange");

    let mut reporter = PortfolioReporter::new(&config.symbol, false, &config.report_path);
    let coingecko = CoinGeckoClient::new();
    let trade_limiter = TradeLimiter::new(&format!(
        "{}/trade_state.json",
        std::env::current_dir()?.display()
    ));

    {
        let status = reporter.status_mut();
        status.stop_loss_percent = config.stop_loss_percent;
        status.take_profit_percent = config.take_profit_percent;
    }

    let balance = exchange.get_balance().await?;
    let balance_map: std::collections::HashMap<String, Decimal> = balance
        .iter()
        .map(|(k, v)| (k.clone(), v.free))
        .collect();
    reporter.update_balances(balance_map);
    info!("💰 Account balance: {:?}", balance);

    let mut last_ai_update = std::time::Instant::now();
    let mut current_targets: Option<AiTradingTargets> = None;

    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("🔄 Starting LIVE monitoring loop...");
    warn!("⚠️ This will execute REAL trades!");
    info!("   Press Ctrl+C to stop");
    info!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    loop {
        // Fetch real price from exchange
        let current_price = match exchange.get_price(&config.symbol).await {
            Ok(p) => p,
            Err(e) => {
                error!("❌ Failed to get price: {}", e);
                tokio::time::sleep(Duration::from_secs(PRICE_CHECK_INTERVAL_SECS)).await;
                continue;
            }
        };

        info!("📊 {} @ ${:.2}", config.symbol, current_price);

        // Update reporter
        if let Some(event) = reporter.update_price(current_price) {
            info!("🔔 ALERT: {}", event);
        }

        // Recalculate targets periodically
        if current_targets.is_none() || last_ai_update.elapsed().as_secs() >= AI_RECALC_INTERVAL_SECS {
            if let Ok(market_data) = coingecko.fetch_market_data(&config.symbol).await {
                let closes: Vec<Decimal> = market_data.hourly_data_24h.iter().map(|d| d.close).collect();
                
                let market_context = MarketContext {
                    symbol: config.symbol.clone(),
                    current_price,
                    high_24h: market_data.high_24h,
                    low_24h: market_data.low_24h,
                    price_change_24h_percent: market_data.price_change_24h_percent,
                    sma_short: strategy::SmaCrossover::calculate_sma(&closes, 10),
                    sma_long: strategy::SmaCrossover::calculate_sma(&closes, 20),
                    rsi: strategy::RsiStrategy::calculate_rsi(&closes, 14),
                    volume_24h: Some(market_data.total_volume),
                    position_entry_price: reporter.status().entry_price,
                    account_balance: reporter.status().total_portfolio_value,
                    hourly_data_summary: Some(coingecko.format_for_ai(&market_data)),
                    high_12h: market_data.hourly_data_12h.iter().map(|h| h.high).max(),
                    low_12h: market_data.hourly_data_12h.iter().map(|h| h.low).min(),
                    high_48h: market_data.hourly_data_48h.iter().map(|h| h.high).max(),
                    low_48h: market_data.hourly_data_48h.iter().map(|h| h.low).min(),
                };

                let fallback = FallbackTargetCalculator::calculate_targets(&market_context);
                current_targets = Some(fallback.clone());
                reporter.update_ai_targets(&fallback);

                {
                    let status = reporter.status_mut();
                    status.high_24h = market_data.high_24h;
                    status.low_24h = market_data.low_24h;
                    status.price_change_24h_percent = market_data.price_change_24h_percent;
                }
            }

            last_ai_update = std::time::Instant::now();
        }

        // Update trade limits
        let trade_status = trade_limiter.get_status();
        reporter.update_trade_limits(
            trade_status.trades_executed,
            trade_status.can_trade,
            if trade_status.can_trade { None } else { Some(trade_status.date.clone()) },
        );

        // In LIVE mode, we only ALERT - don't auto-execute
        if let Some(ref targets) = current_targets {
            if current_price <= targets.stop_loss_price {
                warn!("🚨 STOP-LOSS ALERT: Price ${:.2} <= SL ${:.2}", 
                    current_price, targets.stop_loss_price);
            }
            if current_price >= targets.take_profit_price {
                info!("🎯 TAKE-PROFIT ALERT: Price ${:.2} >= TP ${:.2}", 
                    current_price, targets.take_profit_price);
            }
        }

        reporter.force_write()?;

        tokio::time::sleep(Duration::from_secs(PRICE_CHECK_INTERVAL_SECS)).await;
    }
}
