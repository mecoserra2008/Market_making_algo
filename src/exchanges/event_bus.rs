use tokio::sync::broadcast;
use std::sync::Arc;

use super::websocket::MarketEvent;

/// Event bus for distributing market events to multiple consumers
/// Allows decoupling of WebSocket client from strategy components
pub struct EventBus {
    tx: broadcast::Sender<Arc<MarketEvent>>,
}

impl EventBus {
    /// Create new event bus with buffer capacity
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all subscribers
    pub fn publish(&self, event: MarketEvent) {
        // Ignore if no subscribers (channel full is ok)
        let _ = self.tx.send(Arc::new(event));
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> broadcast::Receiver<Arc<MarketEvent>> {
        self.tx.subscribe()
    }

    /// Get number of active subscribers
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

impl Clone for EventBus {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Trade, Side};

    #[tokio::test]
    async fn test_event_bus() {
        let bus = EventBus::new(100);

        let mut rx = bus.subscribe();

        bus.publish(MarketEvent::Trade {
            symbol: "BTCUSDT".to_string(),
            trade: Trade {
                timestamp: 0,
                price: 50000.0,
                quantity: 1.0,
                side: Side::Buy,
            },
        });

        let event = rx.recv().await.unwrap();
        match event.as_ref() {
            MarketEvent::Trade { symbol, .. } => {
                assert_eq!(symbol, "BTCUSDT");
            }
            _ => panic!("Wrong event type"),
        }
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let bus = EventBus::new(100);

        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        assert_eq!(bus.subscriber_count(), 2);

        bus.publish(MarketEvent::Connected);

        // Both should receive
        assert!(matches!(rx1.recv().await.unwrap().as_ref(), MarketEvent::Connected));
        assert!(matches!(rx2.recv().await.unwrap().as_ref(), MarketEvent::Connected));
    }
}
