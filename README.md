# Crypto Trading Bot 🤖📈

A high-performance cryptocurrency trading bot written in Rust with AI-powered trading target calculations, real-time CoinGecko market data, and intelligent support/resistance analysis.

## Features

- 🚀 **Async/Await** - Built on Tokio for maximum performance
- 📊 **Technical Analysis** - SMA, RSI, support/resistance levels
- 🧠 **AI Advisor** - Ollama integration for intelligent trading targets
- 🦎 **CoinGecko Integration** - Real-time market data (12h/24h/48h hourly data)
- 📐 **Support & Resistance** - Automatic pivot point calculation
- ⚖️ **Trade Limiter** - Maximum 2 trades per day (configurable)
- 🎮 **Simulation Mode** - Test strategies without risking real money
- 🔒 **Secure** - HMAC-SHA256 signed API requests
- 💱 **Binance Support** - Works with Binance and Binance Testnet
- 📝 **Live Portfolio Reports** - Real-time status file updates
- 🖥️ **Systemd Integration** - Run as a service with journalctl logging

## Prerequisites

### System Requirements
- Ubuntu 22.04+ (or similar Linux distribution)
- 16GB RAM recommended (8GB minimum)
- Rust 1.75+ 

### Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### Install System Dependencies
```bash
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev curl git
```

### Install Ollama (Optional - for AI Advisor)
```bash
# Install Ollama
curl -fsSL https://ollama.com/install.sh | sh

# Download a model (choose one based on your RAM)
ollama pull mistral        # 7B - Recommended for 16GB RAM
ollama pull phi3:mini      # 3.8B - Lighter alternative
ollama pull llama3.2:3b    # 3B - Fastest on CPU
```

### CoinGecko API
The bot uses CoinGecko's free API for real-time market data. No API key required for basic usage (rate limited).

**Supported cryptocurrencies:**
- BTC, ETH, BNB, XRP, ADA, SOL, DOT, DOGE, MATIC, LTC, AVAX, LINK, ATOM, UNI, XLM

## Quick Start

### 1. Clone and Build
```bash
cd ~/git/crypto_trading_bot
cargo build --release
```

### 2. Configure Environment
```bash
# For simulation (recommended to start)
cp .env.simulation .env

# Or for live trading
cp .env.example .env
# Edit .env with your Binance API credentials
```

### 3. Run the Bot
```bash
# Run directly
cargo run --release

# Or run the binary
./target/release/crypto_trading_bot
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `EXCHANGE` | Exchange (`binance`, `binance_testnet`, `simulation`) | `simulation` |
| `API_KEY` | Binance API key | - |
| `API_SECRET` | Binance API secret | - |
| `SYMBOL` | Trading pair | `BTCUSDT` |
| `SIMULATION_MODE` | Enable simulation | `true` |
| `SIMULATION_INITIAL_BALANCE` | Starting balance for simulation | `10000` |
| `SIMULATION_PRICE_VOLATILITY` | Price volatility per tick (0.02 = 2%) | `0.02` |
| `STOP_LOSS_PERCENT` | Stop-loss percentage | `-5.0` |
| `TAKE_PROFIT_PERCENT` | Take-profit percentage | `10.0` |
| `REPORT_PATH` | Portfolio status file path | `portfolio_status.txt` |
| `OLLAMA_ENABLED` | Enable AI advisor | `true` |
| `OLLAMA_URL` | Ollama API URL | `http://localhost:11434` |
| `OLLAMA_MODEL` | Ollama model to use | `mistral` |
| `RUST_LOG` | Log level (`trace`, `debug`, `info`, `warn`, `error`) | `info` |

### Example `.env` for Simulation
```bash
SIMULATION_MODE=true
EXCHANGE=simulation
SYMBOL=BTCUSDT
SIMULATION_INITIAL_BALANCE=10000
STOP_LOSS_PERCENT=-5.0
TAKE_PROFIT_PERCENT=10.0
REPORT_PATH=/home/machado/git/crypto_trading_bot/portfolio_status.txt
OLLAMA_ENABLED=true
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=mistral
RUST_LOG=info
```

### Example `.env` for Live Trading
```bash
EXCHANGE=binance
API_KEY=your_api_key_here
API_SECRET=your_api_secret_here
SYMBOL=BTCUSDT
STOP_LOSS_PERCENT=-5.0
TAKE_PROFIT_PERCENT=10.0
REPORT_PATH=/home/machado/git/crypto_trading_bot/portfolio_status.txt
OLLAMA_ENABLED=true
OLLAMA_URL=http://localhost:11434
OLLAMA_MODEL=mistral
RUST_LOG=info
```

## Running as a Systemd Service

### Install the Service
```bash
sudo ./install-service.sh
```

### Service Commands
```bash
# Start the bot
sudo systemctl start crypto-trading-bot

# Stop the bot
sudo systemctl stop crypto-trading-bot

# Check status
sudo systemctl status crypto-trading-bot

# Restart
sudo systemctl restart crypto-trading-bot

# Enable auto-start on boot
sudo systemctl enable crypto-trading-bot

# Disable auto-start
sudo systemctl disable crypto-trading-bot
```

### View Logs (journalctl)
```bash
# Follow live logs
journalctl -u crypto-trading-bot -f

# Last 100 lines
journalctl -u crypto-trading-bot -n 100

# Today's logs
journalctl -u crypto-trading-bot --since today

# Logs from last hour
journalctl -u crypto-trading-bot --since "1 hour ago"

# Export logs to file
journalctl -u crypto-trading-bot --since today > bot_logs.txt
```

### Uninstall the Service
```bash
sudo ./uninstall-service.sh
```

## Portfolio Status Report

The bot generates a real-time portfolio status file (`portfolio_status.txt`) that updates whenever:
- Price targets are hit (stop-loss, take-profit)
- Trades are executed
- Strategy signals change
- AI advisor updates targets

### Report Sections
- **Market Data** - Current price, 24h change, high/low
- **Trading Targets** - Stop-loss, take-profit, buy/sell targets
- **AI Advisor** - AI recommendation, confidence, reasoning
- **Current Position** - Entry price, size, unrealized P&L
- **Balances** - All asset balances
- **Performance** - Realized P&L, win rate, trade statistics
- **Strategy Signals** - SMA, RSI indicators

### Monitor the Report
```bash
# Watch the report file for changes
watch -n 1 cat portfolio_status.txt

# Or use tail
tail -f portfolio_status.txt
```

## Project Structure

```
crypto_trading_bot/
├── Cargo.toml                          # Dependencies
├── .env                                # Active configuration
├── .env.example                        # Example config for live trading
├── .env.simulation                     # Example config for simulation
├── README.md                           # This file
├── portfolio_status.txt                # Live portfolio report
├── trade_state.json                    # Daily trade tracking
├── install-service.sh                  # Systemd installation script
├── uninstall-service.sh                # Systemd uninstall script
├── crypto-trading-bot.service          # Systemd service file
├── crypto-trading-bot-simulation.service # Systemd service (simulation)
└── src/
    ├── main.rs                         # Entry point
    ├── config.rs                       # Configuration management
    ├── exchange.rs                     # Binance API client
    ├── simulation.rs                   # Simulated exchange
    ├── models.rs                       # Data structures
    ├── strategy.rs                     # Trading strategies (SMA, RSI)
    ├── portfolio.rs                    # Portfolio reporter
    ├── ai_advisor.rs                   # Ollama AI integration
    ├── coingecko.rs                    # CoinGecko market data client
    └── trade_limiter.rs                # Daily trade limit enforcement
```

## Trading Rules

### Maximum 2 Trades Per Day
The bot enforces a strict limit of 2 trades per day to prevent overtrading:
- **Trade 1**: Initial position entry
- **Trade 2**: Position exit or adjustment (only if Trade 1 executed)
- Resets automatically at midnight UTC

This is tracked in `trade_state.json` and persists across bot restarts.

## Support & Resistance Calculation

The bot uses the **Pivot Point** method to calculate key price levels from CoinGecko hourly data:

```
Pivot Point (PP) = (High + Low + Close) / 3
Resistance 1 (R1) = 2 × PP - Low
Resistance 2 (R2) = PP + (High - Low)
Support 1 (S1) = 2 × PP - High
Support 2 (S2) = PP - (High - Low)
```

These levels help identify:
- **Buy targets** near support levels
- **Sell targets** near resistance levels
- **Stop-loss** below strong support
- **Take-profit** near or above resistance

## Trading Strategies

### SMA Crossover
Uses short and long Simple Moving Averages:
- **BUY**: Short SMA crosses above Long SMA
- **SELL**: Short SMA crosses below Long SMA

### RSI Strategy
Uses Relative Strength Index (14-period):
- **BUY**: RSI below 30 (oversold)
- **SELL**: RSI above 70 (overbought)

### AI Advisor
When Ollama is enabled, the AI analyzes:
- Current price and 24h/48h momentum from CoinGecko
- Support and resistance levels
- SMA trend direction
- RSI overbought/oversold conditions
- Account balance and position

The AI provides:
- **Recommendation**: STRONG_BUY, BUY, HOLD, SELL, STRONG_SELL
- **Confidence**: 0-100%
- **Stop-Loss/Take-Profit**: Calculated target prices
- **Buy/Sell Targets**: Entry and exit points based on support/resistance
- **Support/Resistance**: Pivot point analysis
- **Reasoning**: Explanation of the analysis

If Ollama is unavailable, a fallback calculator uses traditional technical analysis with pivot points.

## Development

### Build Debug Version
```bash
cargo build
```

### Build Release Version
```bash
cargo build --release
```

### Run Tests
```bash
cargo test
```

### Check for Errors
```bash
cargo check
```

## Troubleshooting

### Ollama Connection Issues
```bash
# Check if Ollama is running
curl http://localhost:11434/api/tags

# Start Ollama service
ollama serve

# Check available models
ollama list
```

### Build Errors
```bash
# Update Rust
rustup update

# Clean and rebuild
cargo clean
cargo build --release
```

### Service Won't Start
```bash
# Check service status
sudo systemctl status crypto-trading-bot

# Check logs for errors
journalctl -u crypto-trading-bot -n 50

# Verify .env file exists
ls -la /home/machado/git/crypto_trading_bot/.env
```

## ⚠️ Disclaimer

**This bot is for educational purposes only.** 

Cryptocurrency trading carries significant risk:
- Never trade with money you can't afford to lose
- Always test thoroughly on simulation/testnet first
- Past performance does not guarantee future results
- The AI advisor provides suggestions, not financial advice

## License

MIT
