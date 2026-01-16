use std::ops::{Sub, SubAssign};

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum OrderType {
    GoodTillCancelled,
    FillAndKill,
    FillOrKill,
    GoodForDay,
    Market,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
pub struct Price(u32);

#[derive(Debug, Eq, PartialEq, Clone, Copy, PartialOrd, Ord)]
pub struct Quantity(pub u32);

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub struct OrderId(u64);

impl Price {
    pub fn new(value: u32) -> Self {
        Price(value)
    }

    pub fn max() -> Self {
        Price(u32::MAX)
    }

    pub fn min() -> Self {
        Price(0)
    }
}

impl OrderId {
    pub fn new(value: u64) -> Self {
        OrderId(value)
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
