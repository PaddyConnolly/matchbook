use crate::{
    Order, OrderError, OrderId, OrderType, Orders, Price, Quantity, Side, Trade, TradeInfo, Trades,
};
use chrono::{Duration, Local, NaiveTime, Timelike};
use std::cmp::Reverse;
use std::collections::BTreeMap;
use std::ops::Index;
use std::sync::{
    Arc, Condvar, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::thread::JoinHandle;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct LevelInfo {
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct LevelInfos(Vec<LevelInfo>);

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct OrderBookLevels {
    bids: LevelInfos,
    asks: LevelInfos,
}

#[derive(Default)]
pub struct Orderbook {
    bids: BTreeMap<Reverse<Price>, Orders>,
    asks: BTreeMap<Price, Orders>,
    orders: Orders,
    trades: Trades,
    shutdown: Arc<AtomicBool>,
    shutdown_cv: Arc<(Mutex<()>, Condvar)>,
    prune_handle: Option<JoinHandle<()>>,
}

impl LevelInfos {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl Index<usize> for LevelInfos {
    type Output = LevelInfo;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl Orderbook {
    pub fn new() -> Orderbook {
        let shutdown = Arc::new(AtomicBool::new(false));
        let shutdown_cv = Arc::new((Mutex::new(()), Condvar::new()));
        Orderbook {
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            orders: Orders::new(),
            trades: Trades::new(),
            shutdown,
            shutdown_cv,
            prune_handle: None,
        }
    }

    pub fn midprice(&self) -> Option<Price> {
        let best_bid = self.bids.keys().next().map(|Reverse(p)| p)?;
        let best_ask = self.asks.keys().next()?;
        Some(Price::new((best_bid.0 + best_ask.0) / 2))
    }

    pub fn add_order(&mut self, order: Order) -> Result<(), OrderError> {
        if self.orders.contains(order.clone().order_id) {
            return Err(OrderError::IdExists);
        }

        match order.order_type {
            OrderType::FillAndKill => {
                if order.order_type == OrderType::FillAndKill
                    && !self.can_match(order.side, order.price)
                {
                    return Err(OrderError::CantMatch);
                }
            }
            OrderType::FillOrKill => {
                if !self.can_fully_fill(order.side, order.price, order.remaining_quantity) {
                    return Err(OrderError::CantFullyFill);
                }
            }
            OrderType::Market => {
                if !self.has_liquidity(order.side) {
                    return Err(OrderError::NoLiquidity);
                }
            }
            _ => {}
        }

        match order.side {
            Side::Buy => {
                if let Some(orders) = self.bids.get_mut(&Reverse(order.clone().price)) {
                    orders.push_back(order.clone());
                    self.orders.push_back(order.clone());
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order.clone());
                    self.bids.insert(Reverse(order.price), orders);
                    self.orders.push_back(order);
                }
            }
            Side::Sell => {
                if let Some(orders) = self.asks.get_mut(&order.price) {
                    orders.push_back(order.clone());
                    self.orders.push_back(order);
                } else {
                    let mut orders = Orders::new();
                    orders.push_back(order.clone());
                    self.asks.insert(order.price, orders);
                    self.orders.push_back(order);
                }
            }
        }

        Ok(())
    }

    pub fn modify_order(
        &mut self,
        order_id: OrderId,
        new_quantity: Quantity,
    ) -> Result<(), OrderError> {
        let (side, price) = {
            let order = self
                .orders
                .get(order_id.clone())
                .ok_or(OrderError::OrderNotFound)?;
            (order.side, order.price)
        };

        // Update in orders
        if let Some(o) = self.orders.get_mut(order_id.clone()) {
            o.remaining_quantity = new_quantity;
        }

        // Update in bids/asks
        match side {
            Side::Buy => {
                if let Some(orders) = self.bids.get_mut(&Reverse(price))
                    && let Some(o) = orders.get_mut(order_id.clone())
                {
                    o.remaining_quantity = new_quantity;
                }
            }
            Side::Sell => {
                if let Some(orders) = self.asks.get_mut(&price)
                    && let Some(o) = orders.get_mut(order_id)
                {
                    o.remaining_quantity = new_quantity;
                }
            }
        }

        Ok(())
    }

    fn can_match(&self, side: Side, price: Price) -> bool {
        match side {
            Side::Buy => {
                if let Some(&ask_price) = self.asks.keys().next() {
                    ask_price <= price
                } else {
                    false
                }
            }
            Side::Sell => {
                if let Some(&Reverse(bid_price)) = self.bids.keys().next() {
                    bid_price >= price
                } else {
                    false
                }
            }
        }
    }

    pub fn can_fully_fill(&self, side: Side, price: Price, remaining_quantity: Quantity) -> bool {
        let available = match side {
            Side::Buy => self
                .asks
                .iter()
                .filter(|(p, _)| **p <= price)
                .flat_map(|(_, orders)| orders.iter())
                .map(|o| o.remaining_quantity.0)
                .sum::<u64>(),
            Side::Sell => self
                .bids
                .iter()
                .filter(|(rp, _)| rp.0 >= price)
                .flat_map(|(_, orders)| orders.iter())
                .map(|o| o.remaining_quantity.0)
                .sum::<u64>(),
        };
        available >= remaining_quantity.0
    }
    pub fn has_liquidity(&self, side: Side) -> bool {
        match side {
            Side::Buy => !self.asks.is_empty(),
            Side::Sell => !self.bids.is_empty(),
        }
    }

    pub fn match_orders(&mut self) {
        // While we have bids and asks
        while let (Some(&Reverse(best_bid_price)), Some(&best_ask_price)) =
            (self.bids.keys().next(), self.asks.keys().next())
        {
            if best_bid_price < best_ask_price {
                break;
            }

            // Get order info and fill amount
            let (bid_id, ask_id, to_fill) = {
                let bid_orders = self.bids.get_mut(&Reverse(best_bid_price)).unwrap();
                let ask_orders = self.asks.get_mut(&best_ask_price).unwrap();
                let bid_order = bid_orders.front_mut().unwrap();
                let ask_order = ask_orders.front_mut().unwrap();

                let to_fill =
                    std::cmp::min(bid_order.remaining_quantity, ask_order.remaining_quantity);
                bid_order.fill(to_fill).ok();
                ask_order.fill(to_fill).ok();

                (
                    bid_order.order_id.clone(),
                    ask_order.order_id.clone(),
                    to_fill,
                )
            }; // borrows end here

            // Record trade
            let trade_price = if best_ask_price == Price::min() {
                best_bid_price
            } else {
                best_ask_price
            };

            self.trades.push(Trade {
                bid_trade: TradeInfo::new(bid_id.clone(), trade_price, to_fill),
                ask_trade: TradeInfo::new(ask_id.clone(), trade_price, to_fill),
            });

            // Remove filled orders and clean up empty levels
            if let Some(bid_orders) = self.bids.get_mut(&Reverse(best_bid_price)) {
                if bid_orders.front().map(|o| o.is_filled()).unwrap_or(false) {
                    bid_orders.pop();
                    self.orders.delete(bid_id.clone());
                }
                if bid_orders.is_empty() {
                    self.bids.remove(&Reverse(best_bid_price));
                }
            }

            if let Some(ask_orders) = self.asks.get_mut(&best_ask_price) {
                if ask_orders.front().map(|o| o.is_filled()).unwrap_or(false) {
                    ask_orders.pop();
                    self.orders.delete(ask_id.clone());
                }
                if ask_orders.is_empty() {
                    self.asks.remove(&best_ask_price);
                }
            }
        }
        // We need to remove FillAndKills with no other side
        let bid_fak_ids: Vec<OrderId> = self
            .bids
            .first_key_value()
            .map(|(_, orders)| {
                orders
                    .iter()
                    .filter(|order| {
                        matches!(order.order_type, OrderType::FillAndKill | OrderType::Market)
                    })
                    .map(|order| order.order_id.clone())
                    .collect()
            })
            .unwrap_or_default();

        let ask_fak_ids: Vec<OrderId> = self
            .asks
            .first_key_value()
            .map(|(_, orders)| {
                orders
                    .iter()
                    .filter(|order| {
                        matches!(order.order_type, OrderType::FillAndKill | OrderType::Market)
                    })
                    .map(|order| order.order_id.clone())
                    .collect()
            })
            .unwrap_or_default();

        for id in bid_fak_ids.into_iter().chain(ask_fak_ids) {
            let _ = self.cancel_order(id);
        }
    }

    pub fn cancel_order(&mut self, order_id: OrderId) -> Result<(), OrderError> {
        let (side, price) = {
            let order = self
                .orders
                .get(order_id.clone())
                .ok_or(OrderError::OrderNotFound)?;
            (order.side, order.price)
        };

        self.orders.delete(order_id.clone());

        match side {
            Side::Buy => {
                if let Some(bids_at_price) = self.bids.get_mut(&Reverse(price)) {
                    bids_at_price.delete(order_id);
                    if bids_at_price.is_empty() {
                        self.bids.remove(&Reverse(price));
                    }
                }
            }
            Side::Sell => {
                if let Some(asks_at_price) = self.asks.get_mut(&price) {
                    asks_at_price.delete(order_id);
                    if asks_at_price.is_empty() {
                        self.asks.remove(&price);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn get_levels(&self) -> OrderBookLevels {
        let bids = LevelInfos(
            self.bids
                .iter()
                .map(|(Reverse(price), orders)| LevelInfo {
                    price: *price,
                    quantity: orders
                        .iter()
                        .map(|order| order.remaining_quantity)
                        .fold(Quantity(0), |acc, q| Quantity(acc.0.saturating_add(q.0))),
                })
                .collect(),
        );

        let asks = LevelInfos(
            self.asks
                .iter()
                .map(|(price, orders)| LevelInfo {
                    price: *price,
                    quantity: orders
                        .iter()
                        .map(|order| order.remaining_quantity)
                        .fold(Quantity(0), |acc, q| Quantity(acc.0.saturating_add(q.0))),
                })
                .collect(),
        );

        OrderBookLevels { bids, asks }
    }

    pub fn trades(&self) -> &Trades {
        &self.trades
    }

    pub fn clear_trades(&mut self) {
        self.trades.clear();
    }
    pub fn prune_good_for_day_orders(&mut self) {
        let gfd_ids: Vec<OrderId> = self
            .orders
            .iter()
            .filter(|o| o.order_type == OrderType::GoodForDay)
            .map(|o| o.order_id.clone())
            .collect();

        for id in gfd_ids {
            let _ = self.cancel_order(id);
        }
    }

    #[allow(dead_code)]
    fn run_prune_thread(
        orderbook: Arc<Mutex<Self>>,
        shutdown: Arc<AtomicBool>,
        shutdown_cv: Arc<(Mutex<()>, Condvar)>,
    ) {
        const MARKET_CLOSE_HOUR: u32 = 16;

        loop {
            let now = Local::now();
            let today_close = now
                .date_naive()
                .and_time(NaiveTime::from_hms_opt(MARKET_CLOSE_HOUR, 0, 0).unwrap());

            let next_close = if now.time().hour() >= MARKET_CLOSE_HOUR {
                today_close + Duration::days(1)
            } else {
                today_close
            };

            let wait_duration = (next_close - now.naive_local())
                .to_std()
                .unwrap_or(std::time::Duration::from_millis(100))
                + std::time::Duration::from_millis(100);

            // Wait until market close or shutdown
            {
                let (lock, cvar) = &*shutdown_cv;
                let guard = lock.lock().unwrap();
                let result = cvar.wait_timeout(guard, wait_duration).unwrap();

                if shutdown.load(Ordering::Acquire) || !result.1.timed_out() {
                    return;
                }
            }

            // Collect GoodForDay order IDs
            let order_ids: Vec<OrderId> = {
                let ob = orderbook.lock().unwrap();
                ob.orders
                    .iter()
                    .filter(|o| o.order_type == OrderType::GoodForDay)
                    .map(|o| o.order_id.clone())
                    .collect()
            };

            // Cancel them

            {
                let mut ob = orderbook.lock().unwrap();
                for id in order_ids {
                    let _ = ob.cancel_order(id);
                }
            }
        }
    }

    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        let (_, cvar) = &*self.shutdown_cv;
        cvar.notify_all();

        if let Some(handle) = self.prune_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for Orderbook {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl OrderBookLevels {
    pub fn bids(&self) -> &[LevelInfo] {
        &self.bids.0
    }
    pub fn asks(&self) -> &[LevelInfo] {
        &self.asks.0
    }
}

impl LevelInfo {
    pub fn price(&self) -> Price {
        self.price
    }
    pub fn quantity(&self) -> Quantity {
        self.quantity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Price;

    fn price(p: u64) -> Price {
        Price::new(p)
    }

    fn qty(q: u64) -> Quantity {
        Quantity(q)
    }

    fn order_id(id: &str) -> OrderId {
        OrderId::new(id.to_string())
    }

    fn buy_order(id: String, p: u64, q: u64) -> Order {
        Order::new(
            OrderId::new(id),
            OrderType::GoodTillCancelled,
            Side::Buy,
            price(p),
            qty(q),
        )
    }

    fn sell_order(id: String, p: u64, q: u64) -> Order {
        Order::new(
            OrderId::new(id),
            OrderType::GoodTillCancelled,
            Side::Sell,
            price(p),
            qty(q),
        )
    }

    fn buy_fak(id: String, p: u64, q: u64) -> Order {
        Order::new(
            OrderId::new(id),
            OrderType::FillAndKill,
            Side::Buy,
            price(p),
            qty(q),
        )
    }

    mod orderbook {
        use super::*;

        #[test]
        fn new_orderbook_is_empty() {
            let ob = Orderbook::new();
            let levels = ob.get_levels();
            assert!(levels.bids.0.is_empty());
            assert!(levels.asks.0.is_empty());
        }

        #[test]
        fn add_buy_order() {
            let mut ob = Orderbook::new();
            ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
            let levels = ob.get_levels();
            assert_eq!(levels.bids.0.len(), 1);
            assert_eq!(levels.bids.0[0].price, price(100));
            assert_eq!(levels.bids.0[0].quantity, qty(50));
        }

        #[test]
        fn add_sell_order() {
            let mut ob = Orderbook::new();
            ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
            let levels = ob.get_levels();
            assert_eq!(levels.asks.0.len(), 1);
            assert_eq!(levels.asks.0[0].price, price(100));
            assert_eq!(levels.asks.0[0].quantity, qty(50));
        }

        #[test]
        fn add_duplicate_id_fails() {
            let mut ob = Orderbook::new();
            ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
            let result = ob.add_order(buy_order("1".to_string(), 110, 60));
            assert!(matches!(result, Err(OrderError::IdExists)));
        }

        #[test]
        fn add_multiple_orders_same_price() {
            let mut ob = Orderbook::new();
            ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
            ob.add_order(buy_order("2".to_string(), 100, 30)).unwrap();
            let levels = ob.get_levels();
            assert_eq!(levels.bids.0.len(), 1);
            assert_eq!(levels.bids.0[0].quantity, qty(80));
        }

        #[test]
        fn add_multiple_orders_different_prices() {
            let mut ob = Orderbook::new();
            ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
            ob.add_order(buy_order("2".to_string(), 110, 30)).unwrap();
            ob.add_order(buy_order("3".to_string(), 90, 20)).unwrap();
            let levels = ob.get_levels();
            assert_eq!(levels.bids.0.len(), 3);
            assert_eq!(levels.bids.0[0].price, price(110));
            assert_eq!(levels.bids.0[1].price, price(100));
            assert_eq!(levels.bids.0[2].price, price(90));
        }

        #[test]
        fn asks_sorted_low_to_high() {
            let mut ob = Orderbook::new();
            ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
            ob.add_order(sell_order("2".to_string(), 110, 30)).unwrap();
            ob.add_order(sell_order("3".to_string(), 90, 20)).unwrap();
            let levels = ob.get_levels();
            assert_eq!(levels.asks.0.len(), 3);
            assert_eq!(levels.asks.0[0].price, price(90));
            assert_eq!(levels.asks.0[1].price, price(100));
            assert_eq!(levels.asks.0[2].price, price(110));
        }

        #[test]
        fn can_match_buy_when_ask_exists_at_or_below() {
            let mut ob = Orderbook::new();
            ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
            assert!(ob.can_match(Side::Buy, price(100)));
            assert!(ob.can_match(Side::Buy, price(110)));
            assert!(!ob.can_match(Side::Buy, price(90)));
        }

        #[test]
        fn can_match_sell_when_bid_exists_at_or_above() {
            let mut ob = Orderbook::new();
            ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
            assert!(ob.can_match(Side::Sell, price(100)));
            assert!(ob.can_match(Side::Sell, price(90)));
            assert!(!ob.can_match(Side::Sell, price(110)));
        }

        #[test]
        fn can_match_false_on_empty_book() {
            let ob = Orderbook::new();
            assert!(!ob.can_match(Side::Buy, price(100)));
            assert!(!ob.can_match(Side::Sell, price(100)));
        }

        #[test]
        fn fak_rejected_when_no_match_possible() {
            let mut ob = Orderbook::new();
            let result = ob.add_order(buy_fak("1".to_string(), 100, 50));
            assert!(matches!(result, Err(OrderError::CantMatch)));
        }

        #[test]
        fn fak_rejected_when_price_doesnt_cross() {
            let mut ob = Orderbook::new();
            ob.add_order(sell_order("1".to_string(), 110, 50)).unwrap();
            let result = ob.add_order(buy_fak("2".to_string(), 100, 50));
            assert!(matches!(result, Err(OrderError::CantMatch)));
        }

        #[test]
        fn fak_accepted_when_can_match() {
            let mut ob = Orderbook::new();
            ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
            let result = ob.add_order(buy_fak("2".to_string(), 100, 50));
            assert!(result.is_ok());
        }

        mod fill_or_kill {
            use super::*;

            fn buy_fok(id: &str, p: u64, q: u64) -> Order {
                Order::new(
                    order_id(id),
                    OrderType::FillOrKill,
                    Side::Buy,
                    price(p),
                    qty(q),
                )
            }

            fn sell_fok(id: &str, p: u64, q: u64) -> Order {
                Order::new(
                    order_id(id),
                    OrderType::FillOrKill,
                    Side::Sell,
                    price(p),
                    qty(q),
                )
            }

            #[test]
            fn fok_rejected_when_empty_book() {
                let mut ob = Orderbook::new();
                let result = ob.add_order(buy_fok("1", 100, 50));
                assert!(matches!(result, Err(OrderError::CantFullyFill)));
            }

            #[test]
            fn fok_rejected_when_insufficient_quantity() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 30)).unwrap();
                let result = ob.add_order(buy_fok("2", 100, 50));
                assert!(matches!(result, Err(OrderError::CantFullyFill)));
            }

            #[test]
            fn fok_rejected_when_price_doesnt_cross() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 110, 100)).unwrap();
                let result = ob.add_order(buy_fok("2", 100, 50));
                assert!(matches!(result, Err(OrderError::CantFullyFill)));
            }

            #[test]
            fn fok_accepted_when_can_fully_fill_exact() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
                let result = ob.add_order(buy_fok("2", 100, 50));
                assert!(result.is_ok());
            }

            #[test]
            fn fok_accepted_when_can_fully_fill_excess_liquidity() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 100)).unwrap();
                let result = ob.add_order(buy_fok("2", 100, 50));
                assert!(result.is_ok());
            }

            #[test]
            fn fok_accepted_across_multiple_price_levels() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 20)).unwrap();
                ob.add_order(sell_order("2".to_string(), 101, 20)).unwrap();
                ob.add_order(sell_order("3".to_string(), 102, 20)).unwrap();
                let result = ob.add_order(buy_fok("4", 102, 50));
                assert!(result.is_ok());
            }

            #[test]
            fn fok_sell_rejected_when_insufficient_bids() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 30)).unwrap();
                let result = ob.add_order(sell_fok("2", 100, 50));
                assert!(matches!(result, Err(OrderError::CantFullyFill)));
            }

            #[test]
            fn fok_sell_accepted_when_sufficient_bids() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(buy_order("2".to_string(), 99, 50)).unwrap();
                let result = ob.add_order(sell_fok("3", 99, 75));
                assert!(result.is_ok());
            }

            #[test]
            fn can_fully_fill_checks_price_constraint() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(sell_order("2".to_string(), 110, 50)).unwrap();
                let result = ob.add_order(buy_fok("3", 100, 75));
                assert!(matches!(result, Err(OrderError::CantFullyFill)));
            }
        }

        mod market_order {
            use super::*;

            fn buy_market(id: &str, q: u64) -> Order {
                Order::new(order_id(id), OrderType::Market, Side::Buy, price(0), qty(q))
            }

            fn sell_market(id: &str, q: u64) -> Order {
                Order::new(
                    order_id(id),
                    OrderType::Market,
                    Side::Sell,
                    price(0),
                    qty(q),
                )
            }

            #[test]
            fn market_buy_rejected_when_no_liquidity() {
                let mut ob = Orderbook::new();
                let result = ob.add_order(buy_market("1", 50));
                assert!(matches!(result, Err(OrderError::NoLiquidity)));
            }

            #[test]
            fn market_sell_rejected_when_no_liquidity() {
                let mut ob = Orderbook::new();
                let result = ob.add_order(sell_market("1", 50));
                assert!(matches!(result, Err(OrderError::NoLiquidity)));
            }

            #[test]
            fn market_buy_accepted_when_asks_exist() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
                let result = ob.add_order(buy_market("2", 50));
                assert!(result.is_ok());
            }

            #[test]
            fn market_sell_accepted_when_bids_exist() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                let result = ob.add_order(sell_market("2", 50));
                assert!(result.is_ok());
            }

            #[test]
            fn market_buy_uses_max_price() {
                let order = buy_market("1", 50);
                assert_eq!(order.price, Price::max());
            }

            #[test]
            fn market_sell_uses_min_price() {
                let order = sell_market("1", 50);
                assert_eq!(order.price, Price::min());
            }

            #[test]
            fn market_order_ignores_specified_price() {
                let order = Order::new(
                    order_id("1"),
                    OrderType::Market,
                    Side::Buy,
                    price(100),
                    qty(50),
                );
                assert_eq!(order.price, Price::max());
            }
        }

        mod prune_good_for_day {
            use super::*;

            fn buy_gfd(id: &str, p: u64, q: u64) -> Order {
                Order::new(
                    order_id(id),
                    OrderType::GoodForDay,
                    Side::Buy,
                    price(p),
                    qty(q),
                )
            }

            fn sell_gfd(id: &str, p: u64, q: u64) -> Order {
                Order::new(
                    order_id(id),
                    OrderType::GoodForDay,
                    Side::Sell,
                    price(p),
                    qty(q),
                )
            }

            #[test]
            fn prune_empty_book_no_panic() {
                let mut ob = Orderbook::new();
                ob.prune_good_for_day_orders();
                assert!(ob.get_levels().bids.is_empty());
                assert!(ob.get_levels().asks.is_empty());
            }

            #[test]
            fn prune_removes_gfd_bids() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_gfd("1", 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 99, 50)).unwrap();
                ob.prune_good_for_day_orders();
                assert!(ob.get_levels().bids.is_empty());
            }

            #[test]
            fn prune_removes_gfd_asks() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_gfd("1", 100, 50)).unwrap();
                ob.add_order(sell_gfd("2", 101, 50)).unwrap();
                ob.prune_good_for_day_orders();
                assert!(ob.get_levels().asks.is_empty());
            }

            #[test]
            fn prune_leaves_gtc_orders() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(sell_order("2".to_string(), 110, 50)).unwrap();
                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 1);
                assert_eq!(levels.asks.len(), 1);
            }

            #[test]
            fn prune_mixed_gtc_and_gfd() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 99, 50)).unwrap();
                ob.add_order(buy_order("3".to_string(), 98, 50)).unwrap();
                ob.add_order(buy_gfd("4", 97, 50)).unwrap();

                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 2);
                assert_eq!(levels.bids[0].price(), price(100));
                assert_eq!(levels.bids[1].price(), price(98));
            }

            #[test]
            fn prune_same_price_level_mixed() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 10)).unwrap();
                ob.add_order(buy_gfd("2", 100, 20)).unwrap();
                ob.add_order(buy_order("3".to_string(), 100, 30)).unwrap();

                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 1);
                assert_eq!(levels.bids[0].quantity(), qty(40));
            }

            #[test]
            fn prune_leaves_fak_orders() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 100)).unwrap();
                ob.add_order(buy_fak("2".to_string(), 100, 50)).unwrap();
                ob.match_orders();

                ob.add_order(buy_gfd("3", 90, 50)).unwrap();
                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert!(levels.bids.is_empty());
            }

            #[test]
            fn prune_leaves_fok_orders() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 99, 50)).unwrap();

                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 1);
                assert_eq!(levels.bids[0].price(), price(100));
            }

            #[test]
            fn prune_after_partial_fill() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 30)).unwrap();
                ob.add_order(buy_gfd("2", 100, 50)).unwrap();
                ob.match_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids[0].quantity(), qty(20));

                ob.prune_good_for_day_orders();

                assert!(ob.get_levels().bids.is_empty());
            }

            #[test]
            fn prune_cleans_up_empty_price_levels() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_gfd("1", 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 99, 50)).unwrap();
                ob.add_order(buy_gfd("3", 98, 50)).unwrap();

                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert!(levels.bids.is_empty());
            }

            #[test]
            fn prune_multiple_times_is_idempotent() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 99, 50)).unwrap();

                ob.prune_good_for_day_orders();
                ob.prune_good_for_day_orders();
                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 1);
                assert_eq!(levels.bids[0].price(), price(100));
            }

            #[test]
            fn prune_does_not_affect_trades() {
                let mut ob = Orderbook::new();
                ob.add_order(sell_order("1".to_string(), 100, 50)).unwrap();
                ob.add_order(buy_gfd("2", 100, 50)).unwrap();
                ob.match_orders();

                assert_eq!(ob.trades().len(), 1);

                ob.prune_good_for_day_orders();

                assert_eq!(ob.trades().len(), 1);
            }

            #[test]
            fn prune_both_sides() {
                let mut ob = Orderbook::new();
                ob.add_order(buy_gfd("1", 100, 50)).unwrap();
                ob.add_order(buy_order("2".to_string(), 99, 50)).unwrap();
                ob.add_order(sell_gfd("3", 110, 50)).unwrap();
                ob.add_order(sell_order("4".to_string(), 111, 50)).unwrap();

                ob.prune_good_for_day_orders();

                let levels = ob.get_levels();
                assert_eq!(levels.bids.len(), 1);
                assert_eq!(levels.bids[0].price(), price(99));
                assert_eq!(levels.asks.len(), 1);
                assert_eq!(levels.asks[0].price(), price(111));
            }
        }
    }
}
