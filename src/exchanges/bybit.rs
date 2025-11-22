use super::*;
use anyhow::{Result, Context, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use std::sync::Arc;
use dashmap::DashMap;
use crate::models::orderbook::OrderBook;

type HmacSha256 = Hmac<Sha256>;

/// Bybit Exchange Connector
/// Supports V5 API for spot and derivatives trading
#[derive(Clone)]
pub struct BybitExchange {
    api_key: String,
    api_secret: String,
    client: Client,
    base_url: String,
    ws_url: String,
    orderbooks: Arc<DashMap<String, Arc<RwLock<OrderBook>>>>,
}

impl BybitExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Self {
        let (base_url, ws_url) = if testnet {
            (
                "https://api-testnet.bybit.com".to_string(),
                "wss://stream-testnet.bybit.com/v5/public/spot".to_string(),
            )
        } else {
            (
                "https://api.bybit.com".to_string(),
                "wss://stream.bybit.com/v5/public/spot".to_string(),
            )
        };

        Self {
            api_key,
            api_secret,
            client: Client::new(),
            base_url,
            ws_url,
            orderbooks: Arc::new(DashMap::new()),
        }
    }

    /// Generate HMAC signature for authentication
    fn generate_signature(&self, timestamp: u128, params: &str) -> String {
        let sign_str = format!("{}{}{}", timestamp, &self.api_key, params);

        let mut mac = HmacSha256::new_from_slice(self.api_secret.as_bytes())
            .expect("HMAC can take key of any size");

        mac.update(sign_str.as_bytes());

        hex::encode(mac.finalize().into_bytes())
    }

    /// Get current timestamp in milliseconds
    fn get_timestamp() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    }

    /// Make authenticated POST request
    async fn post_signed(&self, endpoint: &str, params: Value) -> Result<Value> {
        let timestamp = Self::get_timestamp();
        let params_str = serde_json::to_string(&params)?;

        let signature = self.generate_signature(timestamp, &params_str);

        let url = format!("{}{}", self.base_url, endpoint);

        let response = self.client
            .post(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-SIGN", signature)
            .header("Content-Type", "application/json")
            .json(&params)
            .send()
            .await
            .context("Failed to send request to Bybit")?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Bybit API error: {} - {}", status, text));
        }

        let json: Value = serde_json::from_str(&text)
            .context("Failed to parse Bybit response")?;

        if json["retCode"].as_i64() != Some(0) {
            return Err(anyhow!(
                "Bybit API error: {} - {}",
                json["retCode"],
                json["retMsg"]
            ));
        }

        Ok(json)
    }

    /// Make authenticated GET request
    async fn get_signed(&self, endpoint: &str, params: &[(&str, &str)]) -> Result<Value> {
        let timestamp = Self::get_timestamp();

        let query_string: String = params
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("&");

        let sign_str = format!("{}{}", timestamp, &self.api_key);
        let signature = self.generate_signature(timestamp, &query_string);

        let url = if query_string.is_empty() {
            format!("{}{}", self.base_url, endpoint)
        } else {
            format!("{}{}?{}", self.base_url, endpoint, query_string)
        };

        let response = self.client
            .get(&url)
            .header("X-BAPI-API-KEY", &self.api_key)
            .header("X-BAPI-TIMESTAMP", timestamp.to_string())
            .header("X-BAPI-SIGN", signature)
            .send()
            .await
            .context("Failed to send request to Bybit")?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Bybit API error: {} - {}", status, text));
        }

        let json: Value = serde_json::from_str(&text)
            .context("Failed to parse Bybit response")?;

        if json["retCode"].as_i64() != Some(0) {
            return Err(anyhow!(
                "Bybit API error: {} - {}",
                json["retCode"],
                json["retMsg"]
            ));
        }

        Ok(json)
    }

    /// Get orderbook for a symbol
    pub async fn get_orderbook(&self, symbol: &str) -> Result<Arc<RwLock<OrderBook>>> {
        if let Some(ob) = self.orderbooks.get(symbol) {
            return Ok(ob.clone());
        }

        let orderbook = Arc::new(RwLock::new(OrderBook::new(symbol.to_string())));
        self.orderbooks.insert(symbol.to_string(), orderbook.clone());

        Ok(orderbook)
    }

    /// Fetch current market price
    pub async fn get_market_price(&self, symbol: &str) -> Result<f64> {
        let response = self.client
            .get(format!("{}/v5/market/tickers", self.base_url))
            .query(&[("category", "spot"), ("symbol", symbol)])
            .send()
            .await?;

        let json: Value = response.json().await?;

        json["result"]["list"][0]["lastPrice"]
            .as_str()
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| anyhow!("Failed to parse market price"))
    }

    /// Get recent trades
    pub async fn get_recent_trades(&self, symbol: &str, limit: usize) -> Result<Vec<crate::models::Trade>> {
        let response = self.client
            .get(format!("{}/v5/market/recent-trade", self.base_url))
            .query(&[
                ("category", "spot"),
                ("symbol", symbol),
                ("limit", &limit.to_string()),
            ])
            .send()
            .await?;

        let json: Value = response.json().await?;

        let trades = json["result"]["list"]
            .as_array()
            .ok_or_else(|| anyhow!("No trades found"))?
            .iter()
            .filter_map(|t| {
                Some(crate::models::Trade {
                    timestamp: t["time"].as_str()?.parse().ok()?,
                    price: t["price"].as_str()?.parse().ok()?,
                    quantity: t["size"].as_str()?.parse().ok()?,
                    side: if t["side"].as_str()? == "Buy" {
                        crate::models::Side::Buy
                    } else {
                        crate::models::Side::Sell
                    },
                })
            })
            .collect();

        Ok(trades)
    }
}

#[async_trait]
impl Exchange for BybitExchange {
    async fn place_order(&self, order: NewOrder) -> Result<Order> {
        let side = match order.side {
            OrderSide::Buy => "Buy",
            OrderSide::Sell => "Sell",
        };

        let order_type = match order.order_type {
            OrderType::Limit => "Limit",
            OrderType::Market => "Market",
            OrderType::PostOnly => "Limit",
        };

        let time_in_force = match order.time_in_force {
            TimeInForce::GTC => "GTC",
            TimeInForce::IOC => "IOC",
            TimeInForce::FOK => "FOK",
            TimeInForce::PostOnly => "PostOnly",
        };

        let mut params = json!({
            "category": "spot",
            "symbol": order.symbol,
            "side": side,
            "orderType": order_type,
            "qty": order.quantity.to_string(),
            "timeInForce": time_in_force,
        });

        if let Some(price) = order.price {
            params["price"] = json!(price.to_string());
        }

        let response = self.post_signed("/v5/order/create", params).await?;

        let order_id = response["result"]["orderId"]
            .as_str()
            .ok_or_else(|| anyhow!("No order ID in response"))?
            .to_string();

        Ok(Order {
            order_id,
            symbol: order.symbol,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: 0.0,
            status: OrderStatus::New,
            timestamp: Self::get_timestamp() as i64,
        })
    }

    async fn cancel_order(&self, symbol: &str, order_id: &str) -> Result<()> {
        let params = json!({
            "category": "spot",
            "symbol": symbol,
            "orderId": order_id,
        });

        self.post_signed("/v5/order/cancel", params).await?;

        Ok(())
    }

    async fn cancel_all_orders(&self, symbol: &str) -> Result<()> {
        let params = json!({
            "category": "spot",
            "symbol": symbol,
        });

        self.post_signed("/v5/order/cancel-all", params).await?;

        Ok(())
    }

    async fn get_order(&self, symbol: &str, order_id: &str) -> Result<Order> {
        let response = self.get_signed(
            "/v5/order/realtime",
            &[
                ("category", "spot"),
                ("symbol", symbol),
                ("orderId", order_id),
            ],
        ).await?;

        let order_data = &response["result"]["list"][0];

        let side = match order_data["side"].as_str() {
            Some("Buy") => OrderSide::Buy,
            Some("Sell") => OrderSide::Sell,
            _ => return Err(anyhow!("Invalid order side")),
        };

        let status = match order_data["orderStatus"].as_str() {
            Some("New") => OrderStatus::New,
            Some("PartiallyFilled") => OrderStatus::PartiallyFilled,
            Some("Filled") => OrderStatus::Filled,
            Some("Cancelled") => OrderStatus::Cancelled,
            Some("Rejected") => OrderStatus::Rejected,
            _ => return Err(anyhow!("Invalid order status")),
        };

        Ok(Order {
            order_id: order_id.to_string(),
            symbol: symbol.to_string(),
            side,
            order_type: OrderType::Limit,
            price: order_data["price"].as_str().and_then(|s| s.parse().ok()),
            quantity: order_data["qty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            filled_quantity: order_data["cumExecQty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
            status,
            timestamp: order_data["createdTime"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
        })
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let response = self.get_signed(
            "/v5/order/realtime",
            &[("category", "spot"), ("symbol", symbol)],
        ).await?;

        let orders = response["result"]["list"]
            .as_array()
            .ok_or_else(|| anyhow!("No orders found"))?
            .iter()
            .filter_map(|o| {
                let side = match o["side"].as_str()? {
                    "Buy" => OrderSide::Buy,
                    "Sell" => OrderSide::Sell,
                    _ => return None,
                };

                let status = match o["orderStatus"].as_str()? {
                    "New" => OrderStatus::New,
                    "PartiallyFilled" => OrderStatus::PartiallyFilled,
                    "Filled" => OrderStatus::Filled,
                    "Cancelled" => OrderStatus::Cancelled,
                    "Rejected" => OrderStatus::Rejected,
                    _ => return None,
                };

                Some(Order {
                    order_id: o["orderId"].as_str()?.to_string(),
                    symbol: o["symbol"].as_str()?.to_string(),
                    side,
                    order_type: OrderType::Limit,
                    price: o["price"].as_str().and_then(|s| s.parse().ok()),
                    quantity: o["qty"].as_str().and_then(|s| s.parse().ok())?,
                    filled_quantity: o["cumExecQty"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0),
                    status,
                    timestamp: o["createdTime"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0),
                })
            })
            .collect();

        Ok(orders)
    }

    async fn get_balance(&self) -> Result<Vec<Balance>> {
        let response = self.get_signed(
            "/v5/account/wallet-balance",
            &[("accountType", "SPOT")],
        ).await?;

        let balances = response["result"]["list"][0]["coin"]
            .as_array()
            .ok_or_else(|| anyhow!("No balance found"))?
            .iter()
            .filter_map(|b| {
                Some(Balance {
                    asset: b["coin"].as_str()?.to_string(),
                    free: b["availableToWithdraw"].as_str()?.parse().ok()?,
                    locked: b["locked"].as_str()?.parse().ok()?,
                })
            })
            .collect();

        Ok(balances)
    }
}
