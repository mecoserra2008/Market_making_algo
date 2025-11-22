use super::*;
use anyhow::{Result, Context, anyhow};
use reqwest::Client;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

/// Deribit Exchange Connector
/// Specialized for options trading and delta hedging
#[derive(Clone)]
pub struct DeribitExchange {
    api_key: String,
    api_secret: String,
    client: Client,
    base_url: String,
    access_token: std::sync::Arc<parking_lot::RwLock<Option<String>>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitOption {
    pub instrument_name: String,
    pub option_type: String, // "call" or "put"
    pub strike: f64,
    pub expiration_timestamp: i64,
    pub underlying: String,
    pub mark_price: f64,
    pub mark_iv: f64, // implied volatility
    pub delta: f64,
    pub gamma: f64,
    pub vega: f64,
    pub theta: f64,
}

impl DeribitExchange {
    pub fn new(api_key: String, api_secret: String, testnet: bool) -> Self {
        let base_url = if testnet {
            "https://test.deribit.com/api/v2"
        } else {
            "https://www.deribit.com/api/v2"
        };

        Self {
            api_key,
            api_secret,
            client: Client::new(),
            base_url: base_url.to_string(),
            access_token: std::sync::Arc::new(parking_lot::RwLock::new(None)),
        }
    }

    /// Authenticate and get access token
    async fn authenticate(&self) -> Result<String> {
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "public/auth",
            "params": {
                "grant_type": "client_credentials",
                "client_id": self.api_key,
                "client_secret": self.api_secret,
            }
        });

        let response = self.client
            .post(&format!("{}/public/auth", self.base_url))
            .json(&params)
            .send()
            .await
            .context("Failed to authenticate with Deribit")?;

        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            return Err(anyhow!("Deribit auth error: {}", error));
        }

        let token = json["result"]["access_token"]
            .as_str()
            .ok_or_else(|| anyhow!("No access token in response"))?
            .to_string();

        *self.access_token.write() = Some(token.clone());

        Ok(token)
    }

    /// Get valid access token (authenticate if needed)
    async fn get_token(&self) -> Result<String> {
        if let Some(token) = self.access_token.read().as_ref() {
            return Ok(token.clone());
        }

        self.authenticate().await
    }

    /// Make authenticated JSON-RPC request
    async fn call_method(&self, method: &str, params: Value) -> Result<Value> {
        let token = self.get_token().await?;

        let payload = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let response = self.client
            .post(&self.base_url)
            .bearer_auth(&token)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Deribit method")?;

        let json: Value = response.json().await?;

        if let Some(error) = json.get("error") {
            // Token might be expired, try re-authenticating once
            if error["code"].as_i64() == Some(13009) {
                let new_token = self.authenticate().await?;

                let response = self.client
                    .post(&self.base_url)
                    .bearer_auth(&new_token)
                    .json(&payload)
                    .send()
                    .await?;

                let json: Value = response.json().await?;

                if let Some(error) = json.get("error") {
                    return Err(anyhow!("Deribit error: {}", error));
                }

                return Ok(json["result"].clone());
            }

            return Err(anyhow!("Deribit error: {}", error));
        }

        Ok(json["result"].clone())
    }

    /// Get all available options for an underlying
    pub async fn get_options(&self, underlying: &str) -> Result<Vec<DeribitOption>> {
        let params = json!({
            "currency": underlying,
            "kind": "option",
        });

        let result = self.call_method("public/get_instruments", params).await?;

        let options = result
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response format"))?
            .iter()
            .filter_map(|inst| self.parse_option(inst).ok())
            .collect();

        Ok(options)
    }

    /// Get option Greeks and market data
    pub async fn get_option_data(&self, instrument_name: &str) -> Result<DeribitOption> {
        let params = json!({
            "instrument_name": instrument_name,
        });

        let result = self.call_method("public/ticker", params).await?;

        self.parse_option(&result)
    }

    /// Parse option data from API response
    fn parse_option(&self, data: &Value) -> Result<DeribitOption> {
        Ok(DeribitOption {
            instrument_name: data["instrument_name"]
                .as_str()
                .ok_or_else(|| anyhow!("Missing instrument_name"))?
                .to_string(),
            option_type: data["option_type"]
                .as_str()
                .unwrap_or("call")
                .to_string(),
            strike: data["strike"]
                .as_f64()
                .ok_or_else(|| anyhow!("Missing strike"))?,
            expiration_timestamp: data["expiration_timestamp"]
                .as_i64()
                .unwrap_or(0),
            underlying: data["underlying_index"]
                .as_str()
                .unwrap_or("BTC")
                .to_string(),
            mark_price: data["mark_price"]
                .as_f64()
                .unwrap_or(0.0),
            mark_iv: data["mark_iv"]
                .as_f64()
                .unwrap_or(0.0) / 100.0, // Convert to decimal
            delta: data["greeks"]["delta"]
                .as_f64()
                .unwrap_or(0.0),
            gamma: data["greeks"]["gamma"]
                .as_f64()
                .unwrap_or(0.0),
            vega: data["greeks"]["vega"]
                .as_f64()
                .unwrap_or(0.0),
            theta: data["greeks"]["theta"]
                .as_f64()
                .unwrap_or(0.0),
        })
    }

    /// Find best option for delta hedging
    /// Returns option that provides desired delta with minimum cost
    pub async fn find_hedge_option(
        &self,
        underlying: &str,
        target_delta: f64,
        max_expiry_days: i64,
    ) -> Result<DeribitOption> {
        let options = self.get_options(underlying).await?;

        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let max_expiry = current_time + (max_expiry_days * 24 * 3600);

        // Filter options by expiry and find best match for delta
        let best_option = options
            .into_iter()
            .filter(|opt| opt.expiration_timestamp <= max_expiry)
            .min_by(|a, b| {
                let delta_diff_a = (a.delta - target_delta).abs();
                let delta_diff_b = (b.delta - target_delta).abs();
                delta_diff_a.partial_cmp(&delta_diff_b).unwrap()
            })
            .ok_or_else(|| anyhow!("No suitable option found"))?;

        Ok(best_option)
    }

    /// Calculate required option quantity for delta hedge
    /// Given a spot position, calculate how many options needed to neutralize delta
    pub fn calculate_hedge_quantity(
        spot_position: f64,
        option_delta: f64,
    ) -> f64 {
        if option_delta.abs() < 0.001 {
            return 0.0;
        }

        // Options delta neutralizes spot delta
        // If long spot, need to short call or long put (negative delta)
        -spot_position / option_delta
    }

    /// Get current underlying price
    pub async fn get_underlying_price(&self, underlying: &str) -> Result<f64> {
        let params = json!({
            "instrument_name": format!("{}-PERPETUAL", underlying),
        });

        let result = self.call_method("public/ticker", params).await?;

        result["last_price"]
            .as_f64()
            .ok_or_else(|| anyhow!("Failed to get underlying price"))
    }
}

#[async_trait]
impl Exchange for DeribitExchange {
    async fn place_order(&self, order: NewOrder) -> Result<Order> {
        let side = match order.side {
            OrderSide::Buy => "buy",
            OrderSide::Sell => "sell",
        };

        let order_type = match order.order_type {
            OrderType::Limit => "limit",
            OrderType::Market => "market",
            OrderType::PostOnly => "limit",
        };

        let mut params = json!({
            "instrument_name": order.symbol,
            "amount": order.quantity,
            "type": order_type,
        });

        if let Some(price) = order.price {
            params["price"] = json!(price);
        }

        if order.time_in_force == TimeInForce::PostOnly {
            params["post_only"] = json!(true);
        }

        let method = format!("private/{}", side);
        let result = self.call_method(&method, params).await?;

        let order_data = &result["order"];

        Ok(Order {
            order_id: order_data["order_id"]
                .as_str()
                .ok_or_else(|| anyhow!("No order ID"))?
                .to_string(),
            symbol: order.symbol,
            side: order.side,
            order_type: order.order_type,
            price: order.price,
            quantity: order.quantity,
            filled_quantity: order_data["filled_amount"]
                .as_f64()
                .unwrap_or(0.0),
            status: OrderStatus::New,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64,
        })
    }

    async fn cancel_order(&self, _symbol: &str, order_id: &str) -> Result<()> {
        let params = json!({
            "order_id": order_id,
        });

        self.call_method("private/cancel", params).await?;

        Ok(())
    }

    async fn cancel_all_orders(&self, symbol: &str) -> Result<()> {
        let params = json!({
            "instrument_name": symbol,
        });

        self.call_method("private/cancel_all_by_instrument", params).await?;

        Ok(())
    }

    async fn get_order(&self, _symbol: &str, order_id: &str) -> Result<Order> {
        let params = json!({
            "order_id": order_id,
        });

        let result = self.call_method("private/get_order_state", params).await?;

        let side = match result["direction"].as_str() {
            Some("buy") => OrderSide::Buy,
            Some("sell") => OrderSide::Sell,
            _ => return Err(anyhow!("Invalid order side")),
        };

        let status = match result["order_state"].as_str() {
            Some("open") => OrderStatus::New,
            Some("filled") => OrderStatus::Filled,
            Some("cancelled") => OrderStatus::Cancelled,
            Some("rejected") => OrderStatus::Rejected,
            _ => OrderStatus::New,
        };

        Ok(Order {
            order_id: order_id.to_string(),
            symbol: result["instrument_name"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            side,
            order_type: OrderType::Limit,
            price: result["price"].as_f64(),
            quantity: result["amount"].as_f64().unwrap_or(0.0),
            filled_quantity: result["filled_amount"].as_f64().unwrap_or(0.0),
            status,
            timestamp: result["creation_timestamp"].as_i64().unwrap_or(0),
        })
    }

    async fn get_open_orders(&self, symbol: &str) -> Result<Vec<Order>> {
        let params = json!({
            "instrument_name": symbol,
        });

        let result = self.call_method("private/get_open_orders_by_instrument", params).await?;

        let orders = result
            .as_array()
            .ok_or_else(|| anyhow!("Invalid response"))?
            .iter()
            .filter_map(|o| {
                let side = match o["direction"].as_str()? {
                    "buy" => OrderSide::Buy,
                    "sell" => OrderSide::Sell,
                    _ => return None,
                };

                Some(Order {
                    order_id: o["order_id"].as_str()?.to_string(),
                    symbol: o["instrument_name"].as_str()?.to_string(),
                    side,
                    order_type: OrderType::Limit,
                    price: o["price"].as_f64(),
                    quantity: o["amount"].as_f64()?,
                    filled_quantity: o["filled_amount"].as_f64().unwrap_or(0.0),
                    status: OrderStatus::New,
                    timestamp: o["creation_timestamp"].as_i64().unwrap_or(0),
                })
            })
            .collect();

        Ok(orders)
    }

    async fn get_balance(&self) -> Result<Vec<Balance>> {
        let params = json!({
            "currency": "BTC",
        });

        let result = self.call_method("private/get_account_summary", params).await?;

        let balance = Balance {
            asset: "BTC".to_string(),
            free: result["available_funds"].as_f64().unwrap_or(0.0),
            locked: result["balance"].as_f64().unwrap_or(0.0)
                - result["available_funds"].as_f64().unwrap_or(0.0),
        };

        Ok(vec![balance])
    }
}
