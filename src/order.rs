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

impl Order {
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
