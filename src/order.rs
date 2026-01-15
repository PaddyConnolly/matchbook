use std::cmp::Reverse;
use std::collections::{BTreeMap, VecDeque};
use std::ops::{Sub, SubAssign};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OrderType {
    GoodTillCancelled,
    FillAndKill,
}

#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("Cannot fill order for more than the available quantity")]
    FillOverflow,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
pub struct Price(u32);

#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
pub struct Quantity(u32);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct OrderId(u64);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
struct LevelInfo {
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone)]
struct LevelInfos(Vec<LevelInfo>);

#[derive(Debug, Eq, PartialEq, Clone)]
struct OrderBookLevels {
    bids: LevelInfos,
    asks: LevelInfos,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Order {
    pub order_id: OrderId,
    pub order_type: OrderType,
    pub side: Side,
    pub price: Price,
    pub initial_quantity: Quantity,
    pub remaining_quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Orders(VecDeque<Order>);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct TradeInfo {
    order_id: OrderId,
    price: Price,
    quantity: Quantity,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct Trade {
    pub bid_trade: TradeInfo,
    pub ask_trade: TradeInfo,
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Trades<'a>(Vec<&'a Trade>);

#[derive(Debug, Clone)]
pub struct Orderbook<'a> {
    bids: BTreeMap<Reverse<Price>, Orders>,
    asks: BTreeMap<Price, Orders>,
    orders: Orders,
    trades: Trades<'a>,
}

impl Order {
    pub fn new(
        order_id: OrderId,
        order_type: OrderType,
        side: Side,
        price: Price,
        initial_quantity: Quantity,
    ) -> Order {
        Order {
            order_id,
            order_type,
            side,
            price,
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
}

impl Sub for Quantity {
    type Output = Self;
    fn sub(self, other: Self) -> Self::Output {
        Quantity(self.0.saturating_sub(other.0))
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 = self.0.saturating_sub(rhs.0)
    }
}

impl<'a> Orderbook<'a> {
    fn can_match(&self, side: Side, price: Price) -> bool {
        match side {
            Side::Buy => {
                if let Some(&ask_price) = self.asks.keys().next() {
                    return ask_price <= price;
                } else {
                    false
                }
            }
            Side::Sell => {
                if let Some(&Reverse(bid_price)) = self.bids.keys().next() {
                    return bid_price >= price;
                } else {
                    false
                }
            }
        }
    }

    fn match_orders(&mut self) {
        loop {
            let best_bid_price = match self.bids.keys().next() {
                Some(&Reverse(price)) => price,
                _ => break,
            };

            let best_ask_price = match self.asks.keys().next() {
                Some(&price) => price,
                _ => break,
            };

            if best_bid_price < best_ask_price {
                break;
            }

            if let (Some(bid_orders), Some(ask_orders)) = (
                self.bids.get_mut(&Reverse(best_bid_price)),
                self.asks.get_mut(&best_ask_price),
            ) {
                if let (Some(bid_order), Some(ask_order)) =
                    (bid_orders.front_mut(), ask_orders.front_mut())
                {
                    let bid_id = bid_order.order_id;
                    let ask_id = ask_order.order_id;
                    let to_fill =
                        std::cmp::min(bid_order.remaining_quantity, ask_order.remaining_quantity);

                    bid_order.fill(to_fill).ok();
                    ask_order.fill(to_fill).ok();

                    if bid_order.remaining_quantity == Quantity(0) {
                        bid_orders.pop();
                        self.orders.delete(bid_id);
                    } else if ask_order.remaining_quantity == Quantity(0) {
                        ask_orders.pop();
                        self.orders.delete(ask_id);
                    } else {
                        unimplemented!();
                    }
                }
            }
        }
    }
}
