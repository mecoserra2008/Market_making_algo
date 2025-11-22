# Professional GUI Integration Plan for Market Maker

## Executive Summary

This document outlines a comprehensive plan for integrating a professional-grade web-based graphical user interface (GUI) with the Rust market making system. The GUI will provide real-time monitoring, control, and analytics capabilities while maintaining the high-performance characteristics of the underlying trading engine.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Technology Stack](#technology-stack)
3. [System Components](#system-components)
4. [Backend API Design](#backend-api-design)
5. [Frontend Components](#frontend-components)
6. [Data Flow](#data-flow)
7. [Implementation Phases](#implementation-phases)
8. [Security Considerations](#security-considerations)
9. [Performance Requirements](#performance-requirements)
10. [Deployment Strategy](#deployment-strategy)

---

## Architecture Overview

### Three-Tier Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     PRESENTATION TIER                            │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │   Market     │  │  Position    │  │     Risk     │          │
│  │   Display    │  │  Monitor     │  │   Dashboard  │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                   │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ Performance  │  │    Order     │  │   System     │          │
│  │   Metrics    │  │  Management  │  │   Health     │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│                                                                   │
│         Web Browser (React + TypeScript + TailwindCSS)           │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                  HTTPS + WebSocket (TLS)
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                      APPLICATION TIER                            │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              REST API Server (Axum)                      │   │
│  │                                                           │   │
│  │  GET  /api/v1/status          - System status           │   │
│  │  GET  /api/v1/positions       - Current positions       │   │
│  │  GET  /api/v1/orders          - Order history           │   │
│  │  POST /api/v1/config          - Update configuration    │   │
│  │  POST /api/v1/control/start   - Start trading           │   │
│  │  POST /api/v1/control/stop    - Stop trading            │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │           WebSocket Server (Axum)                        │   │
│  │                                                           │   │
│  │  /ws/market        - Real-time market data              │   │
│  │  /ws/positions     - Position updates                    │   │
│  │  /ws/performance   - Performance metrics                 │   │
│  │  /ws/logs          - System logs                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                   │
│              Rust HTTP/WebSocket Server (Port 8080)              │
└───────────────────────────┬─────────────────────────────────────┘
                            │
                    In-Process Communication
                    (tokio::sync channels)
                            │
┌───────────────────────────▼─────────────────────────────────────┐
│                        BUSINESS TIER                             │
│                                                                   │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │              Market Maker Core Engine                    │   │
│  │                                                           │   │
│  │  - MarketMakerWS (Strategy orchestrator)                │   │
│  │  - BybitWebSocket (Market data)                         │   │
│  │  - EventBus (Event distribution)                         │   │
│  │  - OrderManager (Order tracking)                         │   │
│  │  - RiskManager (Risk monitoring)                         │   │
│  │  - DeltaHedger (Hedging logic)                          │   │
│  │  - CircuitBreaker (Fault tolerance)                      │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                   │
│                    Existing Rust Trading System                  │
└─────────────────────────────────────────────────────────────────┘
```

### Communication Pattern

1. **REST API**: Request/response for configuration and control operations
2. **WebSocket**: Server-push for real-time market data, positions, and metrics
3. **Event-Driven**: Frontend subscribes to relevant WebSocket channels
4. **Stateless Backend**: API server doesn't maintain client session state

---

## Technology Stack

### Frontend

**Framework**: React 18+ with TypeScript
- Industry standard for professional trading UIs
- Strong typing with TypeScript for reliability
- Excellent component ecosystem
- Virtual DOM for efficient updates

**State Management**: Zustand
- Lightweight and performant
- Simple API compared to Redux
- Built-in TypeScript support
- Perfect for real-time trading data

**UI Components**: TailwindCSS + Headless UI
- Professional, customizable design system
- No opinionated styling overhead
- Responsive by default
- Dark mode support built-in

**Charting Library**: Lightweight Charts by TradingView
- Professional-grade financial charts
- High performance for real-time updates
- Candlestick, line, area chart support
- Orderbook depth visualization

**Real-Time Data**: Native WebSocket API
- Low latency for market data
- Automatic reconnection handling
- Message buffering and backpressure

**Data Visualization**: Recharts
- React-native charting library
- Perfect for performance metrics
- Customizable and responsive
- Time series support

**Build Tool**: Vite
- Fast hot module replacement (HMR)
- Optimized production builds
- TypeScript support out of the box

### Backend API Layer

**Web Framework**: Axum
- Built on Tokio (already in use)
- Type-safe routing
- Excellent WebSocket support
- Low overhead and high performance

**Serialization**: Serde (already in use)
- JSON API responses
- Efficient serialization/deserialization

**Authentication**: JWT (JSON Web Tokens)
- Stateless authentication
- Short-lived access tokens
- Refresh token rotation

**API Documentation**: OpenAPI/Swagger
- Auto-generated from Rust types
- Interactive API explorer
- Client SDK generation

### Deployment

**Containerization**: Docker
- Single container with both Rust backend and static frontend
- Nginx for serving static files and reverse proxy
- Environment-based configuration

**Orchestration**: Docker Compose (development) / Kubernetes (production)
- Multi-service coordination
- Health checks and auto-restart
- Secrets management

---

## System Components

### 1. Market Data Display

**Purpose**: Real-time visualization of orderbook and recent trades

**Features**:
- Live orderbook depth chart (bids/asks)
- Top 20 price levels with volume bars
- Recent trades feed (scrolling list)
- Mid price, spread, and spread basis points
- Orderbook imbalance indicator
- VWAP calculation display

**Data Sources**:
- WebSocket: `/ws/market` (real-time orderbook and trades)
- Update frequency: Every orderbook change (100+ Hz)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  BTCUSDT                    Mid: $50,245.50             │
│  Spread: $2.50 (5.0 bps)   Imbalance: 52% Buy         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ASKS (Red)                                             │
│  50,247.00  ████████████░░░░░░░░░░  2.458              │
│  50,246.50  ██████████░░░░░░░░░░░░  1.823              │
│  50,246.00  ███████████████░░░░░░░  3.142              │
│  ─────────────────────────────────────────              │
│  Mid Price: 50,245.50                                   │
│  ─────────────────────────────────────────              │
│  50,245.00  ████████████████░░░░░░  3.845  (Bid)       │
│  50,244.50  ██████████████████░░░░  4.231              │
│  50,244.00  ███████████░░░░░░░░░░░  2.156              │
│  BIDS (Green)                                           │
│                                                          │
├─────────────────────────────────────────────────────────┤
│  RECENT TRADES                                          │
│  14:23:45.234  50,245.50  0.123  BUY   (Green)         │
│  14:23:45.156  50,245.00  0.456  SELL  (Red)           │
│  14:23:44.987  50,246.00  1.234  BUY   (Green)         │
└─────────────────────────────────────────────────────────┘
```

### 2. Position Monitor

**Purpose**: Track current positions, P&L, and exposure

**Features**:
- Current position size (Bybit perpetual)
- Average entry price
- Current mark price
- Unrealized P&L (absolute and percentage)
- Realized P&L (session and total)
- Delta exposure
- Hedge positions (Deribit options)
- Net delta after hedging

**Data Sources**:
- WebSocket: `/ws/positions` (real-time position updates)
- REST: `GET /api/v1/positions` (initial load)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  POSITION SUMMARY                                        │
├─────────────────────────────────────────────────────────┤
│  Bybit BTCUSDT Perpetual                                │
│  Position: +2.5 BTC                                     │
│  Entry Price: $50,000.00                                │
│  Mark Price: $50,245.50                                 │
│  Unrealized P&L: +$613.75 (+1.23%)                     │
│  Realized P&L: +$1,247.50 (Session)                    │
│                                                          │
│  Delta Exposure: +2.5 BTC                               │
├─────────────────────────────────────────────────────────┤
│  Deribit Hedge Positions                                │
│  BTC-28JUN24-52000-P  -5 contracts  Delta: -1.2 BTC    │
│  BTC-28JUN24-48000-C  +3 contracts  Delta: +0.8 BTC    │
│                                                          │
│  Net Delta: +2.1 BTC  (Target: 0.0 BTC)                │
└─────────────────────────────────────────────────────────┘
```

### 3. Risk Dashboard

**Purpose**: Monitor risk metrics and safety limits

**Features**:
- Maximum position limit and current utilization
- Drawdown tracking (current, maximum, limit)
- Margin utilization percentage
- Circuit breaker status
- Risk score (composite metric)
- Alert indicators for breached limits

**Data Sources**:
- WebSocket: `/ws/risk` (real-time risk updates)
- REST: `GET /api/v1/risk` (initial load)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  RISK METRICS                                            │
├─────────────────────────────────────────────────────────┤
│  Circuit Breaker: CLOSED  (Healthy)                     │
│  Risk Score: 3.2 / 10.0                                 │
│                                                          │
│  Position Limit:  2.5 / 5.0 BTC  [████████░░] 50%      │
│  Drawdown:        $245 / $1000   [███░░░░░░░] 24.5%    │
│  Margin Usage:    45.2%          [█████░░░░░] Warning   │
│                                                          │
│  Alerts:                                                 │
│  [WARN] Margin usage above 40% threshold                │
└─────────────────────────────────────────────────────────┘
```

### 4. Performance Metrics

**Purpose**: Track system performance and trading effectiveness

**Features**:
- Latency statistics (P50, P95, P99)
  - Orderbook processing latency
  - Trade processing latency
  - Order placement latency
- Trading metrics
  - Total volume traded (session/daily)
  - Number of trades
  - Average spread captured
  - Fill ratio
- Market making effectiveness
  - VPIN score (flow toxicity)
  - Adverse selection score
  - Inventory turnover
- System health
  - WebSocket connection status
  - Last heartbeat timestamp
  - Error rate

**Data Sources**:
- WebSocket: `/ws/performance` (30-second updates)
- REST: `GET /api/v1/metrics` (historical data)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  PERFORMANCE METRICS                                     │
├─────────────────────────────────────────────────────────┤
│  Latency (milliseconds)                                  │
│  Component         P50    P95    P99    Status          │
│  Orderbook        2.1    5.3    8.7     HEALTHY         │
│  Trades           1.2    2.8    4.1     HEALTHY         │
│  Order Placement  45.2   78.3   95.1    HEALTHY         │
│                                                          │
│  Trading Statistics (Session)                            │
│  Volume Traded:    125.4 BTC                            │
│  Number of Trades: 1,247                                │
│  Avg Spread:       4.2 bps                              │
│  Fill Ratio:       87.3%                                │
│                                                          │
│  Market Quality                                          │
│  VPIN Score:       0.23  [██░░░░░░░░] LOW               │
│  Adverse Select:   0.18  [█░░░░░░░░░] LOW               │
│                                                          │
│  System Health                                           │
│  WebSocket: CONNECTED  Last update: 0.2s ago            │
│  Error Rate: 0.01%                                      │
└─────────────────────────────────────────────────────────┘
```

### 5. Order Management

**Purpose**: View and manage active orders and order history

**Features**:
- Active orders list (live updates)
- Order history (paginated)
- Order details (fill prices, fees, timestamps)
- Manual order cancellation
- Cancel all orders button (emergency)
- Order type filters (limit, market, post-only)
- Side filters (buy, sell)

**Data Sources**:
- WebSocket: `/ws/orders` (real-time order updates)
- REST: `GET /api/v1/orders` (history, paginated)
- REST: `DELETE /api/v1/orders/{id}` (cancel order)
- REST: `DELETE /api/v1/orders/all` (cancel all)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  ORDER MANAGEMENT                                        │
│  [Cancel All Orders]                         Filters: ▼ │
├─────────────────────────────────────────────────────────┤
│  ACTIVE ORDERS (12)                                     │
│                                                          │
│  Time        Side  Price      Size    Filled  Status    │
│  14:23:45    BUY   50,244.00  0.500   0.000   NEW      │
│  14:23:45    BUY   50,243.50  0.500   0.000   NEW      │
│  14:23:45    SELL  50,246.00  0.500   0.000   NEW      │
│  14:23:45    SELL  50,246.50  0.500   0.000   NEW      │
│  ...                                                     │
│                                                          │
├─────────────────────────────────────────────────────────┤
│  ORDER HISTORY (Showing 1-50 of 1,247)     [1][2][3]... │
│                                                          │
│  Time        Side  Price      Size    Fill$    Fee     │
│  14:23:44    BUY   50,245.00  0.123   $6,180   $1.85   │
│  14:23:43    SELL  50,246.00  0.456   $22,912  $6.87   │
│  ...                                                     │
└─────────────────────────────────────────────────────────┘
```

### 6. Configuration Panel

**Purpose**: Adjust trading parameters without restarting

**Features**:
- Strategy parameters
  - Risk aversion (gamma)
  - Volatility estimate
  - Target inventory
  - Number of quote levels
- Risk parameters
  - Maximum position size
  - Maximum drawdown
  - Minimum spread
- Flow detection parameters
  - VPIN threshold
  - Adverse selection threshold
- System parameters
  - Quote refresh interval
  - Hedge check interval
  - Paper trading mode toggle
- Real-time validation
- Apply/Revert buttons
- Configuration export/import

**Data Sources**:
- REST: `GET /api/v1/config` (current configuration)
- REST: `PUT /api/v1/config` (update configuration)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  CONFIGURATION                                           │
│  [Export] [Import] [Revert] [Apply Changes]            │
├─────────────────────────────────────────────────────────┤
│  Strategy Parameters                                     │
│  Risk Aversion (γ):      [0.5      ] (0.1 - 2.0)       │
│  Volatility (σ):         [0.02     ] (0.001 - 0.1)     │
│  Target Inventory:       [0.0      ] BTC               │
│  Quote Levels:           [5        ] (1 - 10)          │
│  Level Spacing:          [0.0002   ] (0.0001 - 0.001)  │
│                                                          │
│  Risk Limits                                             │
│  Max Position:           [5.0      ] BTC               │
│  Max Drawdown:           [1000.00  ] USD               │
│  Min Spread (bps):       [2.0      ]                   │
│                                                          │
│  Flow Detection                                          │
│  VPIN Threshold:         [0.5      ] (0.0 - 1.0)       │
│  Adverse Sel Threshold:  [0.5      ] (0.0 - 1.0)       │
│                                                          │
│  System                                                  │
│  Paper Trading:          [✓] Enabled                    │
│  Quote Refresh:          [500      ] ms                │
│  Hedge Interval:         [1000     ] ms                │
│                                                          │
│  Changes pending: 3 parameters modified                 │
│  [Apply Changes]  [Revert]                              │
└─────────────────────────────────────────────────────────┘
```

### 7. System Health & Control

**Purpose**: Monitor system status and control trading operations

**Features**:
- Trading state (running, stopped, paused)
- Start/Stop/Pause buttons
- WebSocket connection status
- Exchange connectivity status
- Last successful API call timestamps
- Error log summary
- System uptime
- Resource usage (CPU, memory)

**Data Sources**:
- WebSocket: `/ws/health` (real-time health updates)
- REST: `GET /api/v1/status` (system status)
- REST: `POST /api/v1/control/start` (start trading)
- REST: `POST /api/v1/control/stop` (stop trading)
- REST: `POST /api/v1/control/pause` (pause trading)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  SYSTEM CONTROL                                          │
├─────────────────────────────────────────────────────────┤
│  Status: RUNNING                                         │
│  [Stop Trading]  [Pause]                                │
│                                                          │
│  Uptime: 3h 24m 15s                                     │
│  Last Restart: 2024-01-15 11:00:00                      │
│                                                          │
│  Connectivity                                            │
│  Bybit WebSocket:    CONNECTED   (Latency: 45ms)       │
│  Bybit REST API:     HEALTHY     (Last: 1.2s ago)      │
│  Deribit REST API:   HEALTHY     (Last: 5.3s ago)      │
│                                                          │
│  Resources                                               │
│  CPU Usage:  12.3%  [███░░░░░░░]                       │
│  Memory:     445 MB [████░░░░░░]                        │
│                                                          │
│  Recent Errors (Last 24h)                               │
│  Total: 3  [View Details]                               │
└─────────────────────────────────────────────────────────┘
```

### 8. Logs & Alerts

**Purpose**: Real-time logging and alert notifications

**Features**:
- Live log stream (WebSocket)
- Log level filtering (ERROR, WARN, INFO, DEBUG)
- Keyword search
- Alert notifications (toast/banner)
- Alert history
- Export logs functionality

**Data Sources**:
- WebSocket: `/ws/logs` (real-time log stream)
- REST: `GET /api/v1/logs` (historical logs, paginated)

**UI Layout**:
```
┌─────────────────────────────────────────────────────────┐
│  SYSTEM LOGS                                             │
│  Filter: [ERROR ▼]  Search: [___________] [Export]     │
├─────────────────────────────────────────────────────────┤
│  14:23:45.234 [INFO ] Quote placed: BUY 0.5 @ 50,244.00│
│  14:23:45.123 [INFO ] Quote placed: SELL 0.5 @ 50,246.0│
│  14:23:44.987 [WARN ] High VPIN detected: 0.67          │
│  14:23:44.756 [INFO ] Orderbook updated: spread 4.5 bps │
│  14:23:44.234 [INFO ] Position delta: +2.5 BTC          │
│  ...                                                     │
│                                                          │
│  [Auto-scroll]  Showing latest 100 entries              │
└─────────────────────────────────────────────────────────┘
```

---

## Backend API Design

### REST API Endpoints

#### 1. System Status

**GET /api/v1/status**

Response:
```json
{
  "trading_state": "running",
  "uptime_seconds": 12255,
  "last_restart": "2024-01-15T11:00:00Z",
  "version": "0.1.0",
  "connections": {
    "bybit_websocket": {
      "status": "connected",
      "latency_ms": 45,
      "last_message": "2024-01-15T14:23:45.123Z"
    },
    "bybit_rest": {
      "status": "healthy",
      "last_call": "2024-01-15T14:23:44.000Z"
    },
    "deribit_rest": {
      "status": "healthy",
      "last_call": "2024-01-15T14:23:40.000Z"
    }
  },
  "resources": {
    "cpu_percent": 12.3,
    "memory_mb": 445
  }
}
```

#### 2. Positions

**GET /api/v1/positions**

Response:
```json
{
  "bybit": {
    "symbol": "BTCUSDT",
    "position_size": 2.5,
    "entry_price": 50000.0,
    "mark_price": 50245.5,
    "unrealized_pnl": 613.75,
    "unrealized_pnl_percent": 1.23,
    "delta": 2.5
  },
  "deribit": [
    {
      "instrument": "BTC-28JUN24-52000-P",
      "position": -5,
      "delta": -1.2,
      "mark_price": 1250.0
    },
    {
      "instrument": "BTC-28JUN24-48000-C",
      "position": 3,
      "delta": 0.8,
      "mark_price": 3420.0
    }
  ],
  "net_delta": 2.1,
  "realized_pnl_session": 1247.50,
  "realized_pnl_total": 5432.10
}
```

#### 3. Risk Metrics

**GET /api/v1/risk**

Response:
```json
{
  "circuit_breaker": {
    "state": "closed",
    "failure_rate": 0.0,
    "consecutive_failures": 0,
    "total_calls": 1234
  },
  "risk_score": 3.2,
  "position_utilization": {
    "current": 2.5,
    "limit": 5.0,
    "percent": 50.0
  },
  "drawdown": {
    "current": 245.0,
    "maximum": 312.5,
    "limit": 1000.0,
    "percent": 24.5
  },
  "margin_usage_percent": 45.2,
  "alerts": [
    {
      "severity": "warning",
      "message": "Margin usage above 40% threshold",
      "timestamp": "2024-01-15T14:23:45Z"
    }
  ]
}
```

#### 4. Performance Metrics

**GET /api/v1/metrics**

Response:
```json
{
  "latency": {
    "orderbook": {
      "p50_ms": 2.1,
      "p95_ms": 5.3,
      "p99_ms": 8.7,
      "count": 12450
    },
    "trades": {
      "p50_ms": 1.2,
      "p95_ms": 2.8,
      "p99_ms": 4.1,
      "count": 5678
    },
    "order_placement": {
      "p50_ms": 45.2,
      "p95_ms": 78.3,
      "p99_ms": 95.1,
      "count": 1247
    }
  },
  "trading": {
    "volume_traded": 125.4,
    "num_trades": 1247,
    "avg_spread_bps": 4.2,
    "fill_ratio": 87.3
  },
  "market_quality": {
    "vpin_score": 0.23,
    "adverse_selection_score": 0.18
  },
  "error_rate": 0.01
}
```

#### 5. Orders

**GET /api/v1/orders?status=active&limit=50&offset=0**

Response:
```json
{
  "total": 12,
  "orders": [
    {
      "order_id": "order_123456",
      "timestamp": "2024-01-15T14:23:45.123Z",
      "symbol": "BTCUSDT",
      "side": "buy",
      "type": "limit",
      "price": 50244.0,
      "quantity": 0.5,
      "filled_quantity": 0.0,
      "status": "new"
    }
  ]
}
```

**DELETE /api/v1/orders/{order_id}**

Response:
```json
{
  "success": true,
  "order_id": "order_123456",
  "message": "Order cancelled successfully"
}
```

**DELETE /api/v1/orders/all**

Response:
```json
{
  "success": true,
  "cancelled_count": 12,
  "message": "All orders cancelled successfully"
}
```

#### 6. Configuration

**GET /api/v1/config**

Response:
```json
{
  "strategy": {
    "risk_aversion": 0.5,
    "volatility": 0.02,
    "target_inventory": 0.0,
    "num_levels": 5,
    "level_spacing": 0.0002,
    "vpin_threshold": 0.5,
    "adverse_selection_threshold": 0.5
  },
  "risk": {
    "max_position": 5.0,
    "max_drawdown": 1000.0,
    "min_spread_bps": 2.0
  },
  "system": {
    "paper_trading": true,
    "quote_refresh_ms": 500,
    "hedge_interval_ms": 1000
  }
}
```

**PUT /api/v1/config**

Request:
```json
{
  "strategy": {
    "risk_aversion": 0.6
  }
}
```

Response:
```json
{
  "success": true,
  "message": "Configuration updated successfully",
  "updated_fields": ["strategy.risk_aversion"]
}
```

#### 7. Control Operations

**POST /api/v1/control/start**

Response:
```json
{
  "success": true,
  "message": "Trading started",
  "timestamp": "2024-01-15T14:23:45Z"
}
```

**POST /api/v1/control/stop**

Response:
```json
{
  "success": true,
  "message": "Trading stopped, all orders cancelled",
  "timestamp": "2024-01-15T14:23:45Z"
}
```

**POST /api/v1/control/pause**

Response:
```json
{
  "success": true,
  "message": "Trading paused, orders remain active",
  "timestamp": "2024-01-15T14:23:45Z"
}
```

#### 8. Logs

**GET /api/v1/logs?level=INFO&limit=100&offset=0&search=quote**

Response:
```json
{
  "total": 5423,
  "logs": [
    {
      "timestamp": "2024-01-15T14:23:45.234Z",
      "level": "INFO",
      "message": "Quote placed: BUY 0.5 @ 50,244.00",
      "module": "market_maker_ws"
    }
  ]
}
```

### WebSocket Channels

#### 1. Market Data Feed

**Channel**: `/ws/market`

Server Messages:
```json
{
  "type": "orderbook_update",
  "symbol": "BTCUSDT",
  "timestamp": 1705327425123,
  "bids": [
    [50245.0, 3.845],
    [50244.5, 4.231]
  ],
  "asks": [
    [50246.0, 3.142],
    [50246.5, 1.823]
  ],
  "mid_price": 50245.5,
  "spread": 2.5,
  "spread_bps": 5.0,
  "imbalance": 0.52
}
```

```json
{
  "type": "trade",
  "symbol": "BTCUSDT",
  "timestamp": 1705327425234,
  "price": 50245.5,
  "quantity": 0.123,
  "side": "buy"
}
```

#### 2. Position Updates

**Channel**: `/ws/positions`

Server Messages:
```json
{
  "type": "position_update",
  "timestamp": 1705327425234,
  "position_size": 2.5,
  "entry_price": 50000.0,
  "mark_price": 50245.5,
  "unrealized_pnl": 613.75,
  "realized_pnl_session": 1247.50,
  "delta": 2.5,
  "net_delta": 2.1
}
```

#### 3. Performance Metrics

**Channel**: `/ws/performance`

Server Messages (every 30 seconds):
```json
{
  "type": "metrics_update",
  "timestamp": 1705327425000,
  "latency": {
    "orderbook": {"p50_ms": 2.1, "p95_ms": 5.3, "p99_ms": 8.7},
    "trades": {"p50_ms": 1.2, "p95_ms": 2.8, "p99_ms": 4.1},
    "order_placement": {"p50_ms": 45.2, "p95_ms": 78.3, "p99_ms": 95.1}
  },
  "trading": {
    "volume_traded": 125.4,
    "num_trades": 1247
  },
  "market_quality": {
    "vpin_score": 0.23,
    "adverse_selection_score": 0.18
  }
}
```

#### 4. Risk Updates

**Channel**: `/ws/risk`

Server Messages:
```json
{
  "type": "risk_update",
  "timestamp": 1705327425234,
  "circuit_breaker_state": "closed",
  "risk_score": 3.2,
  "position_utilization_percent": 50.0,
  "drawdown_percent": 24.5,
  "margin_usage_percent": 45.2,
  "new_alerts": [
    {
      "severity": "warning",
      "message": "Margin usage above 40% threshold"
    }
  ]
}
```

#### 5. Order Updates

**Channel**: `/ws/orders`

Server Messages:
```json
{
  "type": "order_update",
  "order_id": "order_123456",
  "timestamp": 1705327425234,
  "symbol": "BTCUSDT",
  "side": "buy",
  "price": 50244.0,
  "quantity": 0.5,
  "filled_quantity": 0.123,
  "status": "partially_filled",
  "avg_fill_price": 50244.0
}
```

#### 6. System Logs

**Channel**: `/ws/logs`

Server Messages:
```json
{
  "type": "log",
  "timestamp": 1705327425234,
  "level": "INFO",
  "message": "Quote placed: BUY 0.5 @ 50,244.00",
  "module": "market_maker_ws"
}
```

#### 7. Health Monitoring

**Channel**: `/ws/health`

Server Messages (every 5 seconds):
```json
{
  "type": "health_update",
  "timestamp": 1705327425000,
  "trading_state": "running",
  "connections": {
    "bybit_websocket": "connected",
    "bybit_rest": "healthy",
    "deribit_rest": "healthy"
  },
  "cpu_percent": 12.3,
  "memory_mb": 445
}
```

---

## Data Flow

### Real-Time Data Flow Diagram

```
┌──────────────────────────────────────────────────────────────┐
│                    Bybit Exchange                             │
│         wss://stream.bybit.com/v5/public/spot                │
└────────────────┬─────────────────────────────────────────────┘
                 │
                 │ Market data (orderbook, trades)
                 ▼
┌──────────────────────────────────────────────────────────────┐
│              BybitWebSocket (Rust)                            │
│  - Parse market data                                          │
│  - Update local orderbook                                     │
│  - Emit MarketEvent                                          │
└────────────────┬─────────────────────────────────────────────┘
                 │
                 │ MarketEvent
                 ▼
┌──────────────────────────────────────────────────────────────┐
│                EventBus (Rust)                                │
│  - Broadcast to all subscribers                               │
└────┬──────────┬──────────┬─────────────────────────────────┬┘
     │          │          │                                  │
     │          │          │                                  │
     ▼          ▼          ▼                                  ▼
┌─────────┐ ┌─────────┐ ┌─────────┐                   ┌──────────┐
│ Market  │ │  VPIN   │ │ Adverse │                   │   API    │
│ Maker   │ │ Module  │ │Selection│                   │  Server  │
│   WS    │ │         │ │ Module  │                   │ (Axum)   │
└─────────┘ └─────────┘ └─────────┘                   └────┬─────┘
                                                             │
                                                             │
                                                             ▼
                                                    ┌────────────────┐
                                                    │   WebSocket    │
                                                    │   /ws/market   │
                                                    └───────┬────────┘
                                                            │
                                                            │ JSON
                                                            ▼
                                                    ┌────────────────┐
                                                    │   Frontend     │
                                                    │  (React App)   │
                                                    └────────────────┘
```

### Configuration Update Flow

```
┌────────────────┐                                ┌────────────────┐
│   Frontend     │                                │   API Server   │
│  (React App)   │                                │    (Axum)      │
└────────┬───────┘                                └───────┬────────┘
         │                                                │
         │ PUT /api/v1/config                            │
         │ {"strategy": {"risk_aversion": 0.6}}          │
         ├──────────────────────────────────────────────►│
         │                                                │
         │                                                │ Validate
         │                                                │ config
         │                                                │
         │                                                ▼
         │                                        ┌────────────────┐
         │                                        │ ConfigManager  │
         │                                        │    (Rust)      │
         │                                        └───────┬────────┘
         │                                                │
         │                                                │ Update
         │                                                │ in memory
         │                                                │
         │                                                ▼
         │                                        ┌────────────────┐
         │                                        │  MarketMakerWS │
         │                                        │  (reads config)│
         │                                        └────────────────┘
         │                                                │
         │ 200 OK                                        │
         │ {"success": true}                             │
         │◄──────────────────────────────────────────────┤
         │                                                │
         ▼                                                │
```

---

## Implementation Phases

### Phase 1: Core Infrastructure (Weeks 1-2)

**Rust Backend**:
- [ ] Add Axum web framework dependency
- [ ] Create API server module (`src/api/mod.rs`)
- [ ] Implement basic REST endpoints (status, health)
- [ ] Set up WebSocket server infrastructure
- [ ] Create data models for API responses
- [ ] Implement CORS middleware
- [ ] Add authentication middleware (JWT)

**Frontend**:
- [ ] Initialize React + TypeScript + Vite project
- [ ] Set up TailwindCSS and component library
- [ ] Create basic app layout and routing
- [ ] Implement WebSocket client wrapper
- [ ] Set up Zustand state management
- [ ] Create API client service

**Integration**:
- [ ] Test REST API endpoints with curl/Postman
- [ ] Test WebSocket connections
- [ ] Verify data serialization/deserialization

**Deliverable**: Basic web server serving static frontend, API endpoints responding, WebSocket connections established

### Phase 2: Market Data Visualization (Weeks 3-4)

**Rust Backend**:
- [ ] Implement `/ws/market` WebSocket channel
- [ ] Bridge EventBus to WebSocket clients
- [ ] Add market data REST endpoints
- [ ] Implement efficient orderbook serialization

**Frontend**:
- [ ] Build Orderbook component with depth chart
- [ ] Build Recent Trades component
- [ ] Implement real-time data updates via WebSocket
- [ ] Add market statistics display (spread, imbalance)
- [ ] Optimize rendering performance for high-frequency updates

**Integration**:
- [ ] Test orderbook updates at 100+ Hz
- [ ] Verify no memory leaks in frontend
- [ ] Measure UI rendering performance

**Deliverable**: Live orderbook and trade feed visible in browser

### Phase 3: Position & Risk Monitoring (Weeks 5-6)

**Rust Backend**:
- [ ] Implement `/ws/positions` WebSocket channel
- [ ] Implement `/ws/risk` WebSocket channel
- [ ] Add position and risk REST endpoints
- [ ] Create position tracking aggregator
- [ ] Implement risk metric calculator

**Frontend**:
- [ ] Build Position Monitor component
- [ ] Build Risk Dashboard component
- [ ] Add P&L charts (Recharts)
- [ ] Implement alert system (toast notifications)
- [ ] Add risk gauge visualizations

**Integration**:
- [ ] Test position updates on fills
- [ ] Verify risk calculations match backend
- [ ] Test alert triggers

**Deliverable**: Real-time position and risk monitoring

### Phase 4: Performance & System Health (Weeks 7-8)

**Rust Backend**:
- [ ] Implement `/ws/performance` WebSocket channel
- [ ] Implement `/ws/health` WebSocket channel
- [ ] Add metrics aggregation service
- [ ] Expose latency statistics via API
- [ ] Add system resource monitoring

**Frontend**:
- [ ] Build Performance Metrics component
- [ ] Build System Health component
- [ ] Add latency charts
- [ ] Add connection status indicators
- [ ] Implement health alerts

**Integration**:
- [ ] Test metrics accuracy
- [ ] Verify health monitoring works during outages

**Deliverable**: Comprehensive performance and health monitoring

### Phase 5: Order Management (Weeks 9-10)

**Rust Backend**:
- [ ] Implement `/ws/orders` WebSocket channel
- [ ] Add order management REST endpoints
- [ ] Implement order cancellation logic
- [ ] Add order history pagination
- [ ] Create order search/filter logic

**Frontend**:
- [ ] Build Order Management component
- [ ] Add active orders table
- [ ] Add order history with pagination
- [ ] Implement order cancellation UI
- [ ] Add order filters and search

**Integration**:
- [ ] Test order placement reflection in UI
- [ ] Test order cancellation
- [ ] Verify order history accuracy

**Deliverable**: Full order management interface

### Phase 6: Configuration & Control (Weeks 11-12)

**Rust Backend**:
- [ ] Implement configuration management system
- [ ] Add hot-reload capability for config changes
- [ ] Implement control endpoints (start/stop/pause)
- [ ] Add configuration validation
- [ ] Implement config export/import

**Frontend**:
- [ ] Build Configuration Panel component
- [ ] Add form validation
- [ ] Implement config export/import UI
- [ ] Build System Control component
- [ ] Add confirmation dialogs for critical actions

**Integration**:
- [ ] Test configuration updates without restart
- [ ] Verify control operations (start/stop/pause)
- [ ] Test config validation

**Deliverable**: Live configuration management and system control

### Phase 7: Logging & Advanced Features (Weeks 13-14)

**Rust Backend**:
- [ ] Implement `/ws/logs` WebSocket channel
- [ ] Add log persistence (optional)
- [ ] Implement log search and filtering
- [ ] Add structured logging support
- [ ] Create log export functionality

**Frontend**:
- [ ] Build Logs component with filtering
- [ ] Add log search functionality
- [ ] Implement log export
- [ ] Add keyboard shortcuts
- [ ] Optimize for accessibility

**Integration**:
- [ ] Test log streaming performance
- [ ] Verify search and filtering
- [ ] Test log export

**Deliverable**: Complete logging system

### Phase 8: Testing & Optimization (Weeks 15-16)

**Testing**:
- [ ] End-to-end testing with Playwright
- [ ] Performance testing (load testing API)
- [ ] WebSocket stress testing (many concurrent clients)
- [ ] Browser compatibility testing
- [ ] Mobile responsiveness testing

**Optimization**:
- [ ] Frontend bundle size optimization
- [ ] API response caching
- [ ] WebSocket message compression
- [ ] Database query optimization (if added)
- [ ] Memory leak detection and fixes

**Documentation**:
- [ ] API documentation (OpenAPI/Swagger)
- [ ] User guide for GUI
- [ ] Developer documentation
- [ ] Deployment guide

**Deliverable**: Production-ready GUI with comprehensive testing

---

## Security Considerations

### 1. Authentication & Authorization

**JWT-Based Authentication**:
- Access tokens (15-minute expiration)
- Refresh tokens (7-day expiration)
- Secure token storage (httpOnly cookies)
- Token rotation on refresh

**Implementation**:
```rust
// src/api/auth.rs
pub struct Claims {
    pub sub: String,  // user ID
    pub exp: usize,   // expiration
    pub iat: usize,   // issued at
}

// Middleware for protected routes
pub async fn auth_middleware(
    req: Request<Body>,
    next: Next<Body>,
) -> Result<Response> {
    let token = extract_token(&req)?;
    let claims = validate_token(&token)?;
    // Attach claims to request extensions
    Ok(next.run(req).await)
}
```

**Authorization Levels**:
- `viewer`: Read-only access (positions, metrics, logs)
- `trader`: Can modify configuration, start/stop trading
- `admin`: Full access including user management

### 2. API Security

**Rate Limiting**:
- Per-IP rate limits on REST endpoints
- Token bucket algorithm
- 100 requests/minute for authenticated users
- 10 requests/minute for unauthenticated

**Input Validation**:
- Strict type checking via Rust type system
- Range validation for numeric parameters
- Sanitization of string inputs
- Rejection of invalid JSON

**CORS Configuration**:
```rust
let cors = CorsLayer::new()
    .allow_origin("https://your-domain.com".parse::<HeaderValue>()?)
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([AUTHORIZATION, CONTENT_TYPE])
    .max_age(Duration::from_secs(3600));
```

### 3. WebSocket Security

**Authentication on Connection**:
- Require JWT token in WebSocket handshake
- Validate token before upgrade
- Close connection if token expires

**Message Validation**:
- Validate all client messages
- Ignore invalid/unknown message types
- Rate limit client messages

**Connection Limits**:
- Maximum concurrent connections per user
- Maximum total connections
- Automatic cleanup of stale connections

### 4. Data Protection

**Sensitive Data Handling**:
- Never expose API keys in responses
- Mask sensitive configuration values in logs
- Encrypt sensitive data at rest (if persisted)

**HTTPS/TLS**:
- Mandatory TLS 1.3 for all connections
- Valid SSL certificate (Let's Encrypt)
- HSTS headers for HTTPS enforcement

### 5. Audit Logging

**Security Events**:
- Login attempts (success/failure)
- Configuration changes (who, what, when)
- Control operations (start/stop trading)
- Order cancellations
- API rate limit violations

**Log Format**:
```json
{
  "timestamp": "2024-01-15T14:23:45Z",
  "event_type": "config_change",
  "user_id": "user_123",
  "ip_address": "192.168.1.100",
  "details": {
    "field": "strategy.risk_aversion",
    "old_value": 0.5,
    "new_value": 0.6
  }
}
```

---

## Performance Requirements

### Backend Performance

**API Latency**:
- P50: < 10ms
- P95: < 50ms
- P99: < 100ms

**WebSocket Throughput**:
- Support 1000+ messages/second
- Support 100+ concurrent WebSocket connections
- Message delivery latency < 5ms

**Resource Usage**:
- Memory: < 500 MB (API server)
- CPU: < 20% (idle), < 50% (active trading)

### Frontend Performance

**Initial Load**:
- First Contentful Paint (FCP): < 1.5s
- Time to Interactive (TTI): < 3.5s
- Bundle size: < 500 KB (gzipped)

**Runtime Performance**:
- 60 FPS rendering for all animations
- WebSocket message processing: < 5ms
- React re-render optimization (memoization)

**Memory**:
- No memory leaks from WebSocket connections
- Bounded data structures (max 1000 entries)
- Efficient cleanup of stale data

### Network Efficiency

**Data Compression**:
- Enable gzip/brotli compression for HTTP
- WebSocket message compression (permessage-deflate)

**Caching**:
- Cache static assets (max-age: 1 year)
- Cache API responses where appropriate (ETag)
- Service worker for offline capability (optional)

---

## Deployment Strategy

### Development Environment

**Docker Compose Setup**:
```yaml
version: '3.8'

services:
  market-maker:
    build:
      context: .
      dockerfile: Dockerfile
    ports:
      - "8080:8080"
    environment:
      - RUST_LOG=debug
      - BYBIT_API_KEY=${BYBIT_API_KEY}
      - BYBIT_API_SECRET=${BYBIT_API_SECRET}
      - DERIBIT_API_KEY=${DERIBIT_API_KEY}
      - DERIBIT_API_SECRET=${DERIBIT_API_SECRET}
      - JWT_SECRET=${JWT_SECRET}
    volumes:
      - ./config:/app/config
      - ./logs:/app/logs
    restart: unless-stopped

  nginx:
    image: nginx:alpine
    ports:
      - "443:443"
      - "80:80"
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      - ./ssl:/etc/ssl/certs
      - ./frontend/dist:/usr/share/nginx/html
    depends_on:
      - market-maker
    restart: unless-stopped
```

### Production Deployment

**Dockerfile** (Multi-stage build):
```dockerfile
# Stage 1: Build Rust backend
FROM rust:1.75 as rust-builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

# Stage 2: Build React frontend
FROM node:20 as frontend-builder
WORKDIR /app
COPY frontend/package.json frontend/package-lock.json ./
RUN npm ci
COPY frontend ./
RUN npm run build

# Stage 3: Production image
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy Rust binary
COPY --from=rust-builder /app/target/release/market-maker /app/
# Copy frontend build
COPY --from=frontend-builder /app/dist /app/static

# Expose port
EXPOSE 8080

# Run
CMD ["/app/market-maker"]
```

**Environment Variables**:
```bash
# Exchange API Credentials
BYBIT_API_KEY=your_api_key
BYBIT_API_SECRET=your_api_secret
DERIBIT_API_KEY=your_api_key
DERIBIT_API_SECRET=your_api_secret

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
RUST_LOG=info

# Security
JWT_SECRET=your_random_secret_minimum_32_chars
JWT_EXPIRATION_MINUTES=15

# CORS
ALLOWED_ORIGINS=https://your-domain.com

# TLS (if handled by app, not nginx)
TLS_CERT_PATH=/app/ssl/cert.pem
TLS_KEY_PATH=/app/ssl/key.pem
```

**Nginx Reverse Proxy**:
```nginx
upstream market_maker {
    server market-maker:8080;
}

server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /etc/ssl/certs/cert.pem;
    ssl_certificate_key /etc/ssl/certs/key.pem;
    ssl_protocols TLSv1.3;

    # Frontend static files
    location / {
        root /usr/share/nginx/html;
        try_files $uri $uri/ /index.html;

        # Cache static assets
        location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff|woff2)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }
    }

    # API proxy
    location /api/ {
        proxy_pass http://market_maker;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    # WebSocket proxy
    location /ws/ {
        proxy_pass http://market_maker;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_read_timeout 86400;
    }
}
```

### Kubernetes Deployment (Optional)

**Deployment YAML**:
```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: market-maker
spec:
  replicas: 1
  selector:
    matchLabels:
      app: market-maker
  template:
    metadata:
      labels:
        app: market-maker
    spec:
      containers:
      - name: market-maker
        image: your-registry/market-maker:latest
        ports:
        - containerPort: 8080
        env:
        - name: BYBIT_API_KEY
          valueFrom:
            secretKeyRef:
              name: exchange-credentials
              key: bybit-api-key
        - name: BYBIT_API_SECRET
          valueFrom:
            secretKeyRef:
              name: exchange-credentials
              key: bybit-api-secret
        resources:
          requests:
            memory: "512Mi"
            cpu: "500m"
          limits:
            memory: "1Gi"
            cpu: "1000m"
        livenessProbe:
          httpGet:
            path: /api/v1/status
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /api/v1/status
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 5
---
apiVersion: v1
kind: Service
metadata:
  name: market-maker
spec:
  selector:
    app: market-maker
  ports:
  - port: 80
    targetPort: 8080
  type: LoadBalancer
```

---

## Implementation Checklist

### Backend (Rust)

- [ ] Add Axum, Tower, and related dependencies to Cargo.toml
- [ ] Create `src/api/` module structure
- [ ] Implement REST endpoints for all resources
- [ ] Implement WebSocket server for real-time feeds
- [ ] Create data serialization models (derive Serialize)
- [ ] Add authentication middleware (JWT)
- [ ] Add CORS middleware
- [ ] Add rate limiting middleware
- [ ] Create configuration hot-reload system
- [ ] Implement control operations (start/stop/pause)
- [ ] Add API documentation generation
- [ ] Write integration tests for API endpoints
- [ ] Add logging for all API operations
- [ ] Implement graceful shutdown

### Frontend (React + TypeScript)

- [ ] Initialize project with Vite
- [ ] Set up TypeScript configuration
- [ ] Install and configure TailwindCSS
- [ ] Set up React Router for navigation
- [ ] Create layout components (header, sidebar, main)
- [ ] Implement WebSocket client service
- [ ] Implement REST API client service
- [ ] Set up Zustand stores for state management
- [ ] Build Market Data Display component
- [ ] Build Position Monitor component
- [ ] Build Risk Dashboard component
- [ ] Build Performance Metrics component
- [ ] Build Order Management component
- [ ] Build Configuration Panel component
- [ ] Build System Health component
- [ ] Build Logs & Alerts component
- [ ] Implement authentication flow
- [ ] Add error handling and loading states
- [ ] Implement responsive design (mobile support)
- [ ] Add dark mode support
- [ ] Optimize bundle size
- [ ] Write component unit tests
- [ ] Write E2E tests with Playwright

### DevOps

- [ ] Create Dockerfile (multi-stage build)
- [ ] Create docker-compose.yml for development
- [ ] Set up Nginx configuration
- [ ] Configure SSL/TLS certificates
- [ ] Set up environment variable management
- [ ] Create deployment scripts
- [ ] Set up CI/CD pipeline (GitHub Actions)
- [ ] Configure monitoring (Prometheus/Grafana)
- [ ] Set up log aggregation (ELK stack or similar)
- [ ] Create backup strategy
- [ ] Document deployment process

---

## Conclusion

This comprehensive GUI integration plan provides a professional, production-ready web interface for the market making system. The architecture separates concerns cleanly between presentation, application, and business tiers, enabling independent development and testing of each layer.

### Key Benefits

1. **Real-Time Monitoring**: Sub-10ms latency for market data visualization
2. **Professional UI**: Clean, responsive design suitable for professional trading
3. **Type Safety**: End-to-end type safety with Rust and TypeScript
4. **Performance**: Optimized for high-frequency updates and low resource usage
5. **Security**: JWT authentication, TLS encryption, rate limiting
6. **Maintainability**: Modular architecture with clear separation of concerns
7. **Scalability**: Support for 100+ concurrent WebSocket connections

### Development Timeline

Total estimated time: 16 weeks (4 months) for full implementation with 1 developer

- Phase 1-2: Weeks 1-4 (Core + Market Data)
- Phase 3-4: Weeks 5-8 (Positions + Performance)
- Phase 5-6: Weeks 9-12 (Orders + Configuration)
- Phase 7-8: Weeks 13-16 (Logging + Testing)

### Next Steps

1. Review and approve this plan
2. Set up development environment
3. Begin Phase 1 implementation
4. Iterate based on feedback

This plan ensures a professional, scalable, and maintainable GUI that integrates seamlessly with the high-performance Rust trading engine.
