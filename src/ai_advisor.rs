use anyhow::{anyhow, Result};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Ollama API client for AI-powered trading target calculations
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    options: OllamaOptions,
}

#[derive(Debug, Serialize)]
struct OllamaOptions {
    temperature: f32,
    num_predict: i32,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

/// AI-calculated trading targets
#[derive(Debug, Clone)]
pub struct AiTradingTargets {
    pub stop_loss_price: Decimal,
    pub take_profit_price: Decimal,
    pub buy_target_price: Option<Decimal>,
    pub sell_target_price: Option<Decimal>,
    pub confidence: Decimal,
    pub reasoning: String,
    pub recommendation: TradingRecommendation,
    // Support and resistance levels
    pub support: Option<Decimal>,
    pub strong_support: Option<Decimal>,
    pub resistance: Option<Decimal>,
    pub strong_resistance: Option<Decimal>,
    pub pivot_point: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TradingRecommendation {
    StrongBuy,
    Buy,
    Hold,
    Sell,
    StrongSell,
}

impl std::fmt::Display for TradingRecommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TradingRecommendation::StrongBuy => write!(f, "STRONG BUY"),
            TradingRecommendation::Buy => write!(f, "BUY"),
            TradingRecommendation::Hold => write!(f, "HOLD"),
            TradingRecommendation::Sell => write!(f, "SELL"),
            TradingRecommendation::StrongSell => write!(f, "STRONG SELL"),
        }
    }
}

/// Market data to send to the AI for analysis
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MarketContext {
    pub symbol: String,
    pub current_price: Decimal,
    pub high_24h: Decimal,
    pub low_24h: Decimal,
    pub price_change_24h_percent: Decimal,
    pub sma_short: Option<Decimal>,
    pub sma_long: Option<Decimal>,
    pub rsi: Option<Decimal>,
    pub volume_24h: Option<Decimal>,
    pub position_entry_price: Option<Decimal>,
    pub account_balance: Decimal,
    // Hourly data for support/resistance calculation
    pub hourly_data_summary: Option<String>,
    pub high_12h: Option<Decimal>,
    pub low_12h: Option<Decimal>,
    pub high_48h: Option<Decimal>,
    pub low_48h: Option<Decimal>,
}

impl OllamaClient {
    pub fn new(base_url: Option<&str>, model: Option<&str>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120)) // LLMs can be slow
            .build()?;

        Ok(Self {
            base_url: base_url.unwrap_or("http://localhost:11434").to_string(),
            model: model.unwrap_or("mistral").to_string(),
            client,
        })
    }

    /// Check if Ollama is running and the model is available
    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/api/tags", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    info!("✅ Ollama is running at {}", self.base_url);
                    Ok(true)
                } else {
                    warn!("⚠️ Ollama responded with error: {}", response.status());
                    Ok(false)
                }
            }
            Err(e) => {
                warn!("❌ Cannot connect to Ollama: {}", e);
                Ok(false)
            }
        }
    }

    /// Calculate trading targets using AI
    pub async fn calculate_targets(&self, context: &MarketContext) -> Result<AiTradingTargets> {
        let prompt = self.build_analysis_prompt(context);
        
        info!("🤖 Requesting AI analysis for {} targets...", context.symbol);
        
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            options: OllamaOptions {
                temperature: 0.3, // Lower temperature for more consistent analysis
                num_predict: 1000,
            },
        };

        let url = format!("{}/api/generate", self.base_url);
        
        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow!("Ollama API error: {}", response.status()));
        }

        let ollama_response: OllamaResponse = response.json().await?;
        
        // Parse the AI response
        self.parse_ai_response(&ollama_response.response, context)
    }

    fn build_analysis_prompt(&self, ctx: &MarketContext) -> String {
        let sma_info = match (ctx.sma_short, ctx.sma_long) {
            (Some(short), Some(long)) => {
                let trend = if short > long { "BULLISH (short > long)" } else { "BEARISH (short < long)" };
                format!("SMA(10): {:.2}, SMA(20): {:.2}, Trend: {}", short, long, trend)
            }
            _ => "Not available".to_string(),
        };

        let rsi_info = match ctx.rsi {
            Some(rsi) => {
                let condition = if rsi > dec!(70) {
                    "OVERBOUGHT"
                } else if rsi < dec!(30) {
                    "OVERSOLD"
                } else {
                    "NEUTRAL"
                };
                format!("{:.2} ({})", rsi, condition)
            }
            _ => "Not available".to_string(),
        };

        let position_info = match ctx.position_entry_price {
            Some(entry) => {
                let pnl_percent = ((ctx.current_price - entry) / entry) * dec!(100);
                format!("Entry: ${:.2}, Current P&L: {:.2}%", entry, pnl_percent)
            }
            None => "No open position".to_string(),
        };

        // Add hourly data if available
        let hourly_info = ctx.hourly_data_summary.clone().unwrap_or_else(|| "Not available".to_string());
        
        let price_ranges = format!(
            "12h Range: ${:.2} - ${:.2}, 48h Range: ${:.2} - ${:.2}",
            ctx.low_12h.unwrap_or(ctx.low_24h),
            ctx.high_12h.unwrap_or(ctx.high_24h),
            ctx.low_48h.unwrap_or(ctx.low_24h),
            ctx.high_48h.unwrap_or(ctx.high_24h)
        );

        format!(r#"You are a crypto trading analyst specializing in support and resistance analysis. Analyze the following market data and calculate precise support/resistance levels.

MARKET DATA FOR {symbol}:
- Current Price: ${current_price:.2}
- 24h High: ${high:.2}
- 24h Low: ${low:.2}
- 24h Change: {change:.2}%
- Price Ranges: {price_ranges}
- Moving Averages: {sma}
- RSI (14): {rsi}
- Account Balance: ${balance:.2} USDT

HOURLY PRICE DATA:
{hourly_info}

CURRENT POSITION:
{position}

Calculate support and resistance levels using:
1. Pivot Point method: PP = (High + Low + Close) / 3
2. Support 1: S1 = 2*PP - High
3. Support 2: S2 = PP - (High - Low)
4. Resistance 1: R1 = 2*PP - Low
5. Resistance 2: R2 = PP + (High - Low)

Provide your analysis in EXACTLY this format (use these exact labels):

RECOMMENDATION: [STRONG_BUY/BUY/HOLD/SELL/STRONG_SELL]
CONFIDENCE: [0-100]%
STOP_LOSS: $[price]
TAKE_PROFIT: $[price]
BUY_TARGET: $[price - a good entry point near support]
SELL_TARGET: $[price - a good exit point near resistance]
SUPPORT: $[S1 price]
STRONG_SUPPORT: $[S2 price]
RESISTANCE: $[R1 price]
STRONG_RESISTANCE: $[R2 price]
PIVOT: $[pivot point price]
REASONING: [Your 2-3 sentence explanation including support/resistance analysis]

Rules:
1. ALWAYS calculate and provide support/resistance levels based on 24h/48h data
2. BUY_TARGET should be near a support level for good entry
3. SELL_TARGET should be near a resistance level for good exit
4. Stop-loss should be below strong support
5. Take-profit should be near or above resistance
6. Even for HOLD recommendations, provide buy/sell targets for future reference
7. Provide specific dollar amounts, not percentages"#,
            symbol = ctx.symbol,
            current_price = ctx.current_price,
            high = ctx.high_24h,
            low = ctx.low_24h,
            change = ctx.price_change_24h_percent,
            price_ranges = price_ranges,
            sma = sma_info,
            rsi = rsi_info,
            balance = ctx.account_balance,
            hourly_info = hourly_info,
            position = position_info,
        )
    }

    fn parse_ai_response(&self, response: &str, context: &MarketContext) -> Result<AiTradingTargets> {
        let response_upper = response.to_uppercase();
        
        // Parse recommendation
        let recommendation = if response_upper.contains("STRONG_BUY") || response_upper.contains("STRONG BUY") {
            TradingRecommendation::StrongBuy
        } else if response_upper.contains("STRONG_SELL") || response_upper.contains("STRONG SELL") {
            TradingRecommendation::StrongSell
        } else if response_upper.contains("RECOMMENDATION: BUY") || response_upper.contains("RECOMMENDATION:BUY") {
            TradingRecommendation::Buy
        } else if response_upper.contains("RECOMMENDATION: SELL") || response_upper.contains("RECOMMENDATION:SELL") {
            TradingRecommendation::Sell
        } else {
            TradingRecommendation::Hold
        };

        // Parse confidence
        let confidence = self.extract_percentage(response, "CONFIDENCE")
            .unwrap_or(dec!(50));

        // Parse prices
        let stop_loss = self.extract_price(response, "STOP_LOSS")
            .or_else(|| self.extract_price(response, "STOP LOSS"))
            .unwrap_or_else(|| context.current_price * dec!(0.95)); // Default 5% below

        let take_profit = self.extract_price(response, "TAKE_PROFIT")
            .or_else(|| self.extract_price(response, "TAKE PROFIT"))
            .unwrap_or_else(|| context.current_price * dec!(1.10)); // Default 10% above

        let buy_target = self.extract_price(response, "BUY_TARGET")
            .or_else(|| self.extract_price(response, "BUY TARGET"));

        let sell_target = self.extract_price(response, "SELL_TARGET")
            .or_else(|| self.extract_price(response, "SELL TARGET"));

        // Parse support and resistance levels
        let support = self.extract_price(response, "SUPPORT:");
        let strong_support = self.extract_price(response, "STRONG_SUPPORT")
            .or_else(|| self.extract_price(response, "STRONG SUPPORT"));
        let resistance = self.extract_price(response, "RESISTANCE:");
        let strong_resistance = self.extract_price(response, "STRONG_RESISTANCE")
            .or_else(|| self.extract_price(response, "STRONG RESISTANCE"));
        let pivot = self.extract_price(response, "PIVOT");

        // Extract reasoning
        let reasoning = self.extract_reasoning(response)
            .unwrap_or_else(|| "AI analysis completed".to_string());

        Ok(AiTradingTargets {
            stop_loss_price: stop_loss,
            take_profit_price: take_profit,
            buy_target_price: buy_target,
            sell_target_price: sell_target,
            confidence,
            reasoning,
            recommendation,
            support,
            strong_support,
            resistance,
            strong_resistance,
            pivot_point: pivot,
        })
    }

    fn extract_price(&self, text: &str, label: &str) -> Option<Decimal> {
        // Look for pattern like "STOP_LOSS: $42000" or "STOP_LOSS: 42000"
        let text_upper = text.to_uppercase();
        let label_upper = label.to_uppercase();
        
        if let Some(pos) = text_upper.find(&label_upper) {
            let after_label = &text[pos + label.len()..];
            // Find the number after $ or : 
            let number_str: String = after_label
                .chars()
                .skip_while(|c| !c.is_ascii_digit() && *c != '.')
                .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == ',')
                .filter(|c| *c != ',')
                .collect();
            
            if !number_str.is_empty() {
                return number_str.parse().ok();
            }
        }
        None
    }

    fn extract_percentage(&self, text: &str, label: &str) -> Option<Decimal> {
        let text_upper = text.to_uppercase();
        let label_upper = label.to_uppercase();
        
        if let Some(pos) = text_upper.find(&label_upper) {
            let after_label = &text[pos + label.len()..];
            let number_str: String = after_label
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            
            if !number_str.is_empty() {
                return number_str.parse().ok();
            }
        }
        None
    }

    fn extract_reasoning(&self, text: &str) -> Option<String> {
        let text_upper = text.to_uppercase();
        
        if let Some(pos) = text_upper.find("REASONING:") {
            let after_label = &text[pos + 10..];
            let reasoning: String = after_label
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            
            if !reasoning.is_empty() {
                return Some(reasoning);
            }
        }
        None
    }
}

/// Fallback calculator when Ollama is not available
pub struct FallbackTargetCalculator;

impl FallbackTargetCalculator {
    /// Calculate targets using traditional technical analysis when AI is unavailable
    pub fn calculate_targets(context: &MarketContext) -> AiTradingTargets {
        let current = context.current_price;
        let high = context.high_48h.unwrap_or(context.high_24h);
        let low = context.low_48h.unwrap_or(context.low_24h);
        
        // Calculate support and resistance using pivot points
        let pivot = (high + low + current) / Decimal::from(3);
        let range = high - low;
        
        // R1 = 2 * Pivot - Low, R2 = Pivot + Range
        let resistance = Decimal::from(2) * pivot - low;
        let strong_resistance = pivot + range;
        
        // S1 = 2 * Pivot - High, S2 = Pivot - Range
        let support = Decimal::from(2) * pivot - high;
        let strong_support = pivot - range;
        
        // Determine volatility from 24h range
        let range_24h = context.high_24h - context.low_24h;
        let volatility_percent = if current > Decimal::ZERO {
            (range_24h / current) * dec!(100)
        } else {
            dec!(3) // Default 3%
        };

        // Adjust stop-loss based on volatility (minimum 2%, maximum 5%)
        let stop_loss_percent = (volatility_percent / dec!(2)).min(dec!(5)).max(dec!(2));
        let stop_loss = current * (dec!(1) - stop_loss_percent / dec!(100));

        // Adjust take-profit (1.5x to 3x the stop-loss distance)
        let take_profit_percent = stop_loss_percent * dec!(2);
        let take_profit = current * (dec!(1) + take_profit_percent / dec!(100));

        // Determine recommendation based on indicators
        let (recommendation, confidence) = Self::determine_recommendation(context);

        // Always calculate buy/sell targets based on support/resistance
        // Buy target near support level (good entry)
        let buy_target = Some(support.max(current * dec!(0.97))); // At support or 3% below
        
        // Sell target near resistance level (good exit)
        let sell_target = Some(resistance.min(current * dec!(1.05))); // At resistance or 5% above

        let reasoning = Self::generate_reasoning(context, &recommendation);

        AiTradingTargets {
            stop_loss_price: stop_loss,
            take_profit_price: take_profit,
            buy_target_price: buy_target,
            sell_target_price: sell_target,
            confidence,
            reasoning,
            recommendation,
            support: Some(support),
            strong_support: Some(strong_support),
            resistance: Some(resistance),
            strong_resistance: Some(strong_resistance),
            pivot_point: Some(pivot),
        }
    }

    fn determine_recommendation(ctx: &MarketContext) -> (TradingRecommendation, Decimal) {
        let mut score: i32 = 0;
        let mut factors = 0;

        // RSI analysis
        if let Some(rsi) = ctx.rsi {
            factors += 1;
            if rsi < dec!(30) {
                score += 2; // Oversold = bullish
            } else if rsi < dec!(40) {
                score += 1;
            } else if rsi > dec!(70) {
                score -= 2; // Overbought = bearish
            } else if rsi > dec!(60) {
                score -= 1;
            }
        }

        // SMA trend analysis
        if let (Some(short), Some(long)) = (ctx.sma_short, ctx.sma_long) {
            factors += 1;
            if short > long {
                score += 1; // Bullish crossover
                if short > long * dec!(1.02) {
                    score += 1; // Strong bullish
                }
            } else {
                score -= 1; // Bearish
                if short < long * dec!(0.98) {
                    score -= 1; // Strong bearish
                }
            }
        }

        // 24h momentum
        if ctx.price_change_24h_percent > dec!(5) {
            score += 1;
        } else if ctx.price_change_24h_percent < dec!(-5) {
            score -= 1;
        }
        factors += 1;

        // Calculate confidence based on alignment of factors
        let max_score = factors * 2;
        let confidence = if max_score > 0 {
            (Decimal::from(score.abs()) / Decimal::from(max_score) * dec!(100)).min(dec!(90))
        } else {
            dec!(50)
        };

        let recommendation = match score {
            s if s >= 3 => TradingRecommendation::StrongBuy,
            s if s >= 1 => TradingRecommendation::Buy,
            s if s <= -3 => TradingRecommendation::StrongSell,
            s if s <= -1 => TradingRecommendation::Sell,
            _ => TradingRecommendation::Hold,
        };

        (recommendation, confidence.max(dec!(30)))
    }

    fn generate_reasoning(ctx: &MarketContext, rec: &TradingRecommendation) -> String {
        let mut reasons = Vec::new();

        if let Some(rsi) = ctx.rsi {
            if rsi < dec!(30) {
                reasons.push("RSI indicates oversold conditions");
            } else if rsi > dec!(70) {
                reasons.push("RSI indicates overbought conditions");
            }
        }

        if let (Some(short), Some(long)) = (ctx.sma_short, ctx.sma_long) {
            if short > long {
                reasons.push("SMA shows bullish trend");
            } else {
                reasons.push("SMA shows bearish trend");
            }
        }

        if ctx.price_change_24h_percent > dec!(5) {
            reasons.push("Strong upward momentum in 24h");
        } else if ctx.price_change_24h_percent < dec!(-5) {
            reasons.push("Strong downward momentum in 24h");
        }

        if reasons.is_empty() {
            format!("{} recommendation based on mixed signals", rec)
        } else {
            reasons.join("; ")
        }
    }
}
