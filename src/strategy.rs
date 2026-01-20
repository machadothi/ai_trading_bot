use crate::models::{Kline, Signal};
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

/// Simple Moving Average Crossover Strategy
/// Generates BUY when short MA crosses above long MA
/// Generates SELL when short MA crosses below long MA
#[allow(dead_code)]
pub struct SmaCrossover {
    pub short_period: usize,
    pub long_period: usize,
}

impl SmaCrossover {
    #[allow(dead_code)]
    pub fn new(short_period: usize, long_period: usize) -> Self {
        Self {
            short_period,
            long_period,
        }
    }

    pub fn calculate_sma(prices: &[Decimal], period: usize) -> Option<Decimal> {
        if prices.len() < period {
            return None;
        }

        let sum: Decimal = prices.iter().rev().take(period).sum();
        Some(sum / Decimal::from(period))
    }

    #[allow(dead_code)]
    pub fn generate_signal(&self, klines: &[Kline]) -> Signal {
        let closes: Vec<Decimal> = klines.iter().map(|k| k.close).collect();

        let short_ma = Self::calculate_sma(&closes, self.short_period);
        let long_ma = Self::calculate_sma(&closes, self.long_period);

        match (short_ma, long_ma) {
            (Some(short), Some(long)) => {
                if short > long {
                    Signal::Buy
                } else if short < long {
                    Signal::Sell
                } else {
                    Signal::Hold
                }
            }
            _ => Signal::Hold,
        }
    }
}

/// RSI (Relative Strength Index) Strategy
/// Generates BUY when RSI is below oversold threshold
/// Generates SELL when RSI is above overbought threshold
#[allow(dead_code)]
pub struct RsiStrategy {
    pub period: usize,
    pub oversold: Decimal,
    pub overbought: Decimal,
}

impl RsiStrategy {
    #[allow(dead_code)]
    pub fn new(period: usize, oversold: Decimal, overbought: Decimal) -> Self {
        Self {
            period,
            oversold,
            overbought,
        }
    }

    pub fn calculate_rsi(prices: &[Decimal], period: usize) -> Option<Decimal> {
        if prices.len() < period + 1 {
            return None;
        }

        let mut gains = Decimal::ZERO;
        let mut losses = Decimal::ZERO;

        for i in (prices.len() - period)..prices.len() {
            let change = prices[i] - prices[i - 1];
            if change > Decimal::ZERO {
                gains += change;
            } else {
                losses += change.abs();
            }
        }

        let avg_gain = gains / Decimal::from(period);
        let avg_loss = losses / Decimal::from(period);

        if avg_loss == Decimal::ZERO {
            return Some(dec!(100));
        }

        let rs = avg_gain / avg_loss;
        let rsi = dec!(100) - (dec!(100) / (dec!(1) + rs));

        Some(rsi)
    }

    #[allow(dead_code)]
    pub fn generate_signal(&self, klines: &[Kline]) -> Signal {
        let closes: Vec<Decimal> = klines.iter().map(|k| k.close).collect();

        match Self::calculate_rsi(&closes, self.period) {
            Some(rsi) => {
                if rsi < self.oversold {
                    Signal::Buy
                } else if rsi > self.overbought {
                    Signal::Sell
                } else {
                    Signal::Hold
                }
            }
            None => Signal::Hold,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_sma_calculation() {
        let prices = vec![dec!(10), dec!(20), dec!(30), dec!(40), dec!(50)];
        let sma = SmaCrossover::calculate_sma(&prices, 3);
        assert_eq!(sma, Some(dec!(40))); // (30 + 40 + 50) / 3 = 40
    }

    #[test]
    fn test_rsi_calculation() {
        // Simple test case
        let prices: Vec<Decimal> = vec![
            dec!(44), dec!(44.34), dec!(44.09), dec!(43.61), dec!(44.33),
            dec!(44.83), dec!(45.10), dec!(45.42), dec!(45.84), dec!(46.08),
            dec!(45.89), dec!(46.03), dec!(45.61), dec!(46.28), dec!(46.28),
        ];
        
        let rsi = RsiStrategy::calculate_rsi(&prices, 14);
        assert!(rsi.is_some());
    }
}
