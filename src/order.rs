use crate::{OrderError, OrderId, OrderType, Price, Quantity, Side};
use std::collections::VecDeque;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
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
        if let Some(order) = self.get(order_id) {
            self.0.contains(order)
        } else {
            false
        }
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
