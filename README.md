# State-of-the-Art Market Making Algorithm for Bybit

A professional-grade market making system for Bybit cryptocurrency exchange with Deribit options hedging, implementing cutting-edge algorithms for informed flow detection and optimal quoting.

## 🚀 Features

### Core Market Making
- **Avellaneda-Stoikov Model**: Optimal market making strategy balancing inventory risk and spread profit
- **Multi-level Order Stacking**: Strategic liquidity provision across multiple price levels
- **Dynamic Spread Adjustment**: Adaptive spreads based on market conditions

### Advanced Flow Detection
- **VPIN (Volume-Synchronized Probability of Informed Trading)**: Real-time flow toxicity detection
- **Adverse Selection Detection**: Multi-signal detection system including:
  - Post-trade price movement analysis
  - Fill rate imbalance monitoring
  - Realized spread tracking
  - Book Exhaustion Rate (BER) calculation

### Delta-Neutral Hedging
- **Cross-Exchange Hedging**: Use Deribit options to hedge Bybit spot positions
- **Automatic Delta Management**: Maintains delta-neutral portfolio
- **Greeks Monitoring**: Track delta, gamma, vega exposure
- **Smart Hedge Selection**: Optimal option selection for cost-efficient hedging

### Risk Management
- **Position Limits**: Configurable position and order size limits
- **Drawdown Protection**: Automatic halt on maximum drawdown breach
- **Real-time P&L Tracking**: Continuous monitoring of realized and unrealized P&L
- **Sharpe Ratio Calculation**: Performance metrics tracking

## 📊 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Market Maker Core                       │
│  ┌─────────────────────────────────────────────────────┐   │
│  │         Avellaneda-Stoikov Strategy                 │   │
│  │  • Reservation Price Calculation                    │   │
│  │  • Optimal Spread Determination                     │   │
│  │  • Multi-level Quote Generation                     │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │     VPIN     │  │   Adverse    │  │     LOB      │    │
│  │   Detector   │  │  Selection   │  │   Modeling   │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└─────────────────────────────────────────────────────────────┘
                            │
        ┌───────────────────┴───────────────────┐
        ▼                                       ▼
┌──────────────────┐                  ┌──────────────────┐
│  Bybit Exchange  │                  │ Deribit Exchange │
│  • Spot Trading  │                  │ • Options Trading│
│  • Limit Orders  │                  │ • Delta Hedging  │
│  • WebSocket     │                  │ • Greeks Data    │
└──────────────────┘                  └──────────────────┘
```

## 🛠️ Installation

### Prerequisites
- Rust 1.75 or higher
- Bybit API credentials
- Deribit API credentials (optional, for hedging)

### Setup

1. Clone the repository:
```bash
git clone <repository-url>
cd Market_making_algo
```

2. Copy the example environment file:
```bash
cp .env.example .env
```

3. Edit `.env` and add your API credentials:
```bash
BYBIT_API_KEY=your_api_key
BYBIT_API_SECRET=your_api_secret
DERIBIT_API_KEY=your_deribit_key
DERIBIT_API_SECRET=your_deribit_secret
```

4. Configure strategy parameters in `config/config.toml`

5. Build the project:
```bash
cargo build --release
```

## 🚦 Usage

### Running the Market Maker

```bash
cargo run --release
```

### Testing

Run the test suite:
```bash
cargo test
```

Run with specific log level:
```bash
RUST_LOG=debug cargo run --release
```

## ⚙️ Configuration

### Strategy Parameters

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `risk_aversion` | Controls spread width (higher = wider) | 0.5 | 0.1 - 2.0 |
| `inventory_target` | Target inventory position | 0.0 | Any |
| `min_spread_bps` | Minimum spread in basis points | 5.0 | 1.0+ |
| `max_spread_bps` | Maximum spread in basis points | 50.0 | 10.0+ |
| `order_quantity` | Base order size | 0.01 | Any |
| `num_levels` | Number of price levels | 5 | 1-10 |
| `vpin_threshold` | Flow toxicity threshold | 0.7 | 0.0 - 1.0 |
| `adverse_selection_threshold` | Adverse selection threshold | 0.6 | 0.0 - 1.0 |

### Risk Parameters

| Parameter | Description | Default |
|-----------|-------------|---------|
| `max_position` | Maximum position size | 1.0 |
| `max_drawdown` | Maximum drawdown (USD) | 1000.0 |
| `max_order_value` | Maximum order value (USD) | 10000.0 |
| `delta_hedge_threshold` | Delta to trigger hedge | 0.2 |
| `vega_limit` | Maximum vega exposure | 1000.0 |
| `gamma_limit` | Maximum gamma exposure | 100.0 |

## 📈 Models & Algorithms

### Avellaneda-Stoikov Model

The core market making strategy is based on the seminal Avellaneda-Stoikov (2008) model:

**Reservation Price:**
```
r = s - q·γ·σ²·(T-t)
```

**Optimal Spread:**
```
δ = γ·σ²·(T-t) + (2/γ)·ln(1 + γ/k)
```

Where:
- `s` = mid price
- `q` = current inventory
- `γ` = risk aversion parameter
- `σ` = volatility
- `T-t` = time remaining

### VPIN (Flow Toxicity)

VPIN measures order flow imbalance to detect informed trading:

```
VPIN = Σ|V_buy - V_sell| / Σ(V_buy + V_sell)
```

High VPIN values (>0.7) indicate toxic flow, triggering:
- Wider spreads
- Reduced order sizes
- Increased caution

### Adverse Selection Detection

Multi-signal detection system:

1. **Realized Spread**: Post-trade price movement
2. **Fill Imbalance**: One-sided execution detection
3. **Flow Toxicity**: Volume-weighted trade direction
4. **Book Exhaustion Rate**: Liquidity consumption speed

### Delta Hedging

Maintains delta-neutral portfolio using:
- Deribit perpetual futures (simple, efficient)
- Deribit options (sophisticated, controls gamma/vega)

**Hedge Quantity:**
```
Q_hedge = -Δ_spot / Δ_option
```

## 📚 Research Sources

This implementation is based on cutting-edge research:

### Academic Papers
- [Avellaneda & Stoikov (2008)](https://www.researchgate.net/publication/24086205_High_Frequency_Trading_in_a_Limit_Order_Book) - High-frequency trading in a limit order book
- [Easley, López de Prado & O'Hara (2010)](https://www.stern.nyu.edu/sites/default/files/assets/documents/con_035928.pdf) - Flow Toxicity and Liquidity
- [Reinforcement Learning for Market Making (2021)](https://dl.acm.org/doi/10.1145/3490354.3494398) - Adverse selection risk control
- [Online Learning of Order Flow (2024)](https://www.tandfonline.com/doi/full/10.1080/14697688.2024.2337300) - Bayesian change-point detection

### Industry Resources
- [DWF Labs Market Making Strategies](https://www.dwf-labs.com/news/4-common-strategies-that-crypto-market-makers-use)
- [Deribit Delta Hedging Guide](https://insights.deribit.com/industry/how-to-use-delta-hedging-to-lock-up-profits/)
- [Yellow Capital Advanced Algorithms](https://www.yellowcapital.com/blog/advanced-crypto-market-making-algorithms-for-trading)

## ⚠️ Disclaimers

- **Use at your own risk**: This software is provided "as-is" without warranty
- **Not financial advice**: This is educational/research software
- **Test thoroughly**: Always test on testnet before live trading
- **Monitor closely**: Market making requires active monitoring
- **Capital risk**: You can lose money trading

## 🔐 Security

- Never commit API keys to version control
- Use environment variables for sensitive data
- Enable IP whitelisting on exchange APIs
- Use separate API keys for testing and production
- Review all configuration before live trading

## 📝 License

This project is provided for educational and research purposes.

## 🤝 Contributing

Contributions are welcome! Areas for improvement:
- WebSocket integration for real-time data
- Machine learning for parameter optimization
- Additional exchanges support
- Enhanced Greeks modeling
- Backtesting framework

## 📞 Support

For issues and questions:
- Open an issue on GitHub
- Review the documentation
- Check the example configuration

## 🎯 Roadmap

- [ ] WebSocket real-time orderbook
- [ ] Machine learning for spread optimization
- [ ] Backtesting engine
- [ ] Multi-pair market making
- [ ] Advanced options strategies (gamma scalping, etc.)
- [ ] Performance analytics dashboard
- [ ] Paper trading mode

---

**Built with cutting-edge market making technology for optimal performance in cryptocurrency markets.**
