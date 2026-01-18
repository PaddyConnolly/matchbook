use crate::{OrderError, OrderId, OrderType, Price, Quantity, Side};
use std::collections::VecDeque;

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Order {
    pub order_id: OrderId,
    pub order_type: OrderType,
    pub side: Side,
    pub price: Price,
    pub initial_quantity: Quantity,
    pub remaining_quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone, Default)]
pub struct Orders(VecDeque<Order>);

impl Order {
    pub fn new(
        order_id: OrderId,
        order_type: OrderType,
        side: Side,
        price: Price,
        initial_quantity: Quantity,
    ) -> Order {
        // Market orders use extreme prices to ensure they match
        let effective_price = if order_type == OrderType::Market {
            match side {
                Side::Buy => Price::max(),  // willing to pay anything
                Side::Sell => Price::min(), // willing to sell at any price
            }
        } else {
            price
        };

        Order {
            order_id,
            order_type,
            side,
            price: effective_price,
            initial_quantity,
            remaining_quantity: initial_quantity,
        }
    }

    pub fn filled_quantity(&self) -> Quantity {
        self.initial_quantity - self.remaining_quantity
    }

    pub fn fill(&mut self, quantity: Quantity) -> Result<(), OrderError> {
        if quantity > self.remaining_quantity {
            return Err(OrderError::FillOverflow);
        }
        self.remaining_quantity -= quantity;
        Ok(())
    }

    pub fn is_filled(&self) -> bool {
        self.remaining_quantity == Quantity(0)
    }
}

impl Orders {
    pub fn new() -> Self {
        Orders(VecDeque::new())
    }

    pub fn get(&self, order_id: OrderId) -> Option<&Order> {
        self.0.iter().find(|&order| order.order_id == order_id)
    }

    pub fn get_mut(&mut self, order_id: OrderId) -> Option<&mut Order> {
        self.0.iter_mut().find(|order| order.order_id == order_id)
    }

    pub fn pop(&mut self) -> Option<Order> {
        self.0.pop_front()
    }

    pub fn front(&self) -> Option<&Order> {
        self.0.front()
    }

    pub fn front_mut(&mut self) -> Option<&mut Order> {
        self.0.front_mut()
    }

    pub fn delete(&mut self, order_id: OrderId) {
        if let Some(pos) = self.0.iter().position(|order| order.order_id == order_id) {
            self.0.remove(pos);
        }
    }

    pub fn contains(&self, order_id: OrderId) -> bool {
        self.get(order_id).is_some()
    }

    pub fn push_back(&mut self, order: Order) {
        self.0.push_back(order);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Order> {
        self.0.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrderId, OrderType, Price, Quantity, Side};

    fn price(p: u64) -> Price {
        Price::new(p)
    }

    fn qty(q: u64) -> Quantity {
        Quantity(q)
    }

    fn order_id(id: &str) -> OrderId {
        OrderId::new(id.to_string())
    }

    fn buy_order(id: &str, p: u64, quantity: u64) -> Order {
        Order::new(
            order_id(id),
            OrderType::GoodTillCancelled,
            Side::Buy,
            price(p),
            qty(quantity),
        )
    }
    mod order_tests {
        use super::*;

        #[test]
        fn new_order_has_full_remaining_quantity() {
            let order = buy_order("1", 100, 50);
            assert_eq!(order.remaining_quantity, qty(50));
            assert_eq!(order.initial_quantity, qty(50));
        }

        #[test]
        fn filled_quantity_starts_at_zero() {
            let order = buy_order("1", 100, 50);
            assert_eq!(order.filled_quantity(), qty(0));
        }

        #[test]
        fn fill_reduces_remaining_quantity() {
            let mut order = buy_order("1", 100, 50);
            order.fill(qty(20)).unwrap();
            assert_eq!(order.remaining_quantity, qty(30));
            assert_eq!(order.filled_quantity(), qty(20));
        }

        #[test]
        fn fill_entire_order() {
            let mut order = buy_order("1", 100, 50);
            order.fill(qty(50)).unwrap();
            assert_eq!(order.remaining_quantity, qty(0));
            assert!(order.is_filled());
        }

        #[test]
        fn fill_overflow_returns_error() {
            let mut order = buy_order("1", 100, 50);
            let result = order.fill(qty(51));
            assert!(matches!(result, Err(OrderError::FillOverflow)));
        }

        #[test]
        fn partial_fills_accumulate() {
            let mut order = buy_order("1", 100, 100);
            order.fill(qty(30)).unwrap();
            order.fill(qty(30)).unwrap();
            order.fill(qty(30)).unwrap();
            assert_eq!(order.remaining_quantity, qty(10));
            assert_eq!(order.filled_quantity(), qty(90));
        }

        #[test]
        fn is_filled_false_when_remaining() {
            let mut order = buy_order("1", 100, 50);
            order.fill(qty(49)).unwrap();
            assert!(!order.is_filled());
        }

        #[test]
        fn zero_quantity_order() {
            let order = buy_order("1", 100, 0);
            assert!(order.is_filled());
        }

        #[test]
        fn fill_zero_quantity() {
            let mut order = buy_order("1", 100, 50);
            order.fill(qty(0)).unwrap();
            assert_eq!(order.remaining_quantity, qty(50));
        }
    }

    mod orders_tests {
        use super::*;

        #[test]
        fn new_orders_is_empty() {
            let orders = Orders::new();
            assert!(orders.is_empty());
            assert!(orders.front().is_none());
        }

        #[test]
        fn push_back_adds_order() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            assert!(!orders.is_empty());
            assert!(orders.contains(order_id("1")));
        }

        #[test]
        fn front_returns_first_order() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            orders.push_back(buy_order("2", 100, 60));
            let front = orders.front().unwrap();
            assert_eq!(front.order_id, order_id("1"));
        }

        #[test]
        fn pop_removes_first_order() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            orders.push_back(buy_order("2", 100, 60));
            let popped = orders.pop().unwrap();
            assert_eq!(popped.order_id, order_id("1"));
            assert_eq!(orders.front().unwrap().order_id, order_id("2"));
        }

        #[test]
        fn get_finds_order_by_id() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            orders.push_back(buy_order("2", 100, 60));
            orders.push_back(buy_order("3", 100, 70));
            let order = orders.get(order_id("2")).unwrap();
            assert_eq!(order.initial_quantity, qty(60));
        }

        #[test]
        fn get_returns_none_for_missing_id() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            assert!(orders.get(order_id("999")).is_none());
        }

        #[test]
        fn delete_removes_order() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            orders.push_back(buy_order("2", 100, 60));
            orders.delete(order_id("1"));
            assert!(!orders.contains(order_id("1")));
            assert!(orders.contains(order_id("2")));
        }

        #[test]
        fn delete_nonexistent_is_noop() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 50));
            orders.delete(order_id("999"));
            assert!(orders.contains(order_id("1")));
        }

        #[test]
        fn fifo_ordering_preserved() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 10));
            orders.push_back(buy_order("2", 100, 20));
            orders.push_back(buy_order("3", 100, 30));

            assert_eq!(orders.pop().unwrap().order_id, order_id("1"));
            assert_eq!(orders.pop().unwrap().order_id, order_id("2"));
            assert_eq!(orders.pop().unwrap().order_id, order_id("3"));
        }

        #[test]
        fn iter_yields_all_orders() {
            let mut orders = Orders::new();
            orders.push_back(buy_order("1", 100, 10));
            orders.push_back(buy_order("2", 100, 20));
            orders.push_back(buy_order("3", 100, 30));

            let ids: Vec<_> = orders.iter().map(|o| o.order_id.clone()).collect();
            assert_eq!(ids, vec![order_id("1"), order_id("2"), order_id("3")]);
        }
    }
}
