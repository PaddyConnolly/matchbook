// tests/integration.rs

use matchbook::{Order, OrderError, OrderId, OrderType, Orderbook, Price, Quantity, Side};

fn price(p: u32) -> Price {
    Price::new(p)
}

fn qty(q: u32) -> Quantity {
    Quantity(q)
}

fn order_id(id: u64) -> OrderId {
    OrderId::new(id)
}

fn buy_order(id: u64, p: u32, q: u32) -> Order {
    Order::new(
        order_id(id),
        OrderType::GoodTillCancelled,
        Side::Buy,
        price(p),
        qty(q),
    )
}

fn sell_order(id: u64, p: u32, q: u32) -> Order {
    Order::new(
        order_id(id),
        OrderType::GoodTillCancelled,
        Side::Sell,
        price(p),
        qty(q),
    )
}

fn buy_fak(id: u64, p: u32, q: u32) -> Order {
    Order::new(
        order_id(id),
        OrderType::FillAndKill,
        Side::Buy,
        price(p),
        qty(q),
    )
}

fn sell_fak(id: u64, p: u32, q: u32) -> Order {
    Order::new(
        order_id(id),
        OrderType::FillAndKill,
        Side::Sell,
        price(p),
        qty(q),
    )
}

// ============== Matching tests ==============

mod matching {
    use super::*;

    #[test]
    fn no_match_when_bid_below_ask() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 110, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(50));
        assert_eq!(levels.asks()[0].quantity(), qty(50));
    }

    #[test]
    fn exact_match_removes_both_orders() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn partial_fill_bid_larger() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 100)).unwrap();
        ob.add(sell_order(2, 100, 40)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(60));
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn partial_fill_ask_larger() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 40)).unwrap();
        ob.add(sell_order(2, 100, 100)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(60));
    }

    #[test]
    fn match_when_bid_higher_than_ask() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 110, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn multiple_matches_in_sequence() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 30)).unwrap();
        ob.add(sell_order(2, 100, 30)).unwrap();
        ob.add(buy_order(3, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(10));
    }

    #[test]
    fn matches_best_price_first() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 90, 25)).unwrap();
        ob.add(sell_order(2, 100, 25)).unwrap();
        ob.add(buy_order(3, 100, 30)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks().len(), 1);
        assert_eq!(levels.asks()[0].price(), price(100));
        assert_eq!(levels.asks()[0].quantity(), qty(20));
    }

    #[test]
    fn fifo_matching_same_price() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.add(buy_order(3, 100, 60)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(40));
    }

    #[test]
    fn match_clears_multiple_price_levels() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 10)).unwrap();
        ob.add(sell_order(2, 101, 10)).unwrap();
        ob.add(sell_order(3, 102, 10)).unwrap();
        ob.add(buy_order(4, 105, 25)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks().len(), 1);
        assert_eq!(levels.asks()[0].price(), price(102));
        assert_eq!(levels.asks()[0].quantity(), qty(5));
    }
}

// ============== Fill and Kill tests ==============

mod fill_and_kill {
    use super::*;

    #[test]
    fn fak_fully_filled() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        ob.add(buy_fak(2, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn fak_partial_fill_remainder_cancelled() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 30)).unwrap();
        ob.add(buy_fak(2, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn fak_sell_works() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_fak(2, 100, 30)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(20));
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn fak_cancelled_when_no_liquidity_remains() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 20)).unwrap();
        ob.add(sell_order(2, 110, 20)).unwrap();
        ob.add(buy_fak(3, 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        // FAK can only match 20 @ 100, rest cancelled
        // sell @ 110 remains
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks().len(), 1);
        assert_eq!(levels.asks()[0].price(), price(110));
    }
}

// ============== Cancel order tests ==============

mod cancel {
    use super::*;

    #[test]
    fn cancel_existing_buy_order() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.cancel_order(order_id(1)).unwrap();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
    }

    #[test]
    fn cancel_existing_sell_order() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        ob.cancel_order(order_id(1)).unwrap();
        let levels = ob.get_levels();
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn cancel_nonexistent_order_fails() {
        let mut ob = Orderbook::new();
        let result = ob.cancel_order(order_id(999));
        assert!(matches!(result, Err(OrderError::OrderNotFound)));
    }

    #[test]
    fn cancel_one_of_many_at_price() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(buy_order(2, 100, 30)).unwrap();
        ob.add(buy_order(3, 100, 20)).unwrap();
        ob.cancel_order(order_id(2)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(70));
    }

    #[test]
    fn cancel_removes_empty_price_level() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(buy_order(2, 110, 30)).unwrap();
        ob.cancel_order(order_id(1)).unwrap();
        let levels = ob.get_levels();
        assert_eq!(levels.bids().len(), 1);
        assert_eq!(levels.bids()[0].price(), price(110));
    }

    #[test]
    fn cancel_same_order_twice_fails() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.cancel_order(order_id(1)).unwrap();
        let result = ob.cancel_order(order_id(1));
        assert!(matches!(result, Err(OrderError::OrderNotFound)));
    }
}

// ============== End-to-end scenarios ==============

mod scenarios {
    use super::*;

    #[test]
    fn typical_trading_session() {
        let mut ob = Orderbook::new();

        // Market makers post quotes
        ob.add(buy_order(1, 99, 100)).unwrap();
        ob.add(buy_order(2, 98, 100)).unwrap();
        ob.add(sell_order(3, 101, 100)).unwrap();
        ob.add(sell_order(4, 102, 100)).unwrap();

        // Aggressive buyer crosses spread
        ob.add(buy_order(5, 101, 50)).unwrap();
        ob.match_orders();

        let levels = ob.get_levels();
        assert_eq!(levels.asks()[0].quantity(), qty(50)); // 50 filled @ 101
        assert_eq!(levels.bids()[0].price(), price(99)); // best bid unchanged
    }

    #[test]
    fn orderbook_depth_maintained() {
        let mut ob = Orderbook::new();

        for i in 0..10 {
            ob.add(buy_order(i, 100 - i as u32, 10)).unwrap();
            ob.add(sell_order(100 + i, 110 + i as u32, 10)).unwrap();
        }

        let levels = ob.get_levels();
        assert_eq!(levels.bids().len(), 10);
        assert_eq!(levels.asks().len(), 10);

        // Best bid is 100, best ask is 110
        assert_eq!(levels.bids()[0].price(), price(100));
        assert_eq!(levels.asks()[0].price(), price(110));
    }

    #[test]
    fn large_order_sweeps_book() {
        let mut ob = Orderbook::new();

        ob.add(sell_order(1, 100, 10)).unwrap();
        ob.add(sell_order(2, 101, 20)).unwrap();
        ob.add(sell_order(3, 102, 30)).unwrap();
        ob.add(sell_order(4, 103, 40)).unwrap();

        // Large buy sweeps through multiple levels
        ob.add(buy_order(5, 103, 75)).unwrap();
        ob.match_orders();

        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        // 10 + 20 + 30 = 60 filled, 15 remaining from order @ 103
        assert_eq!(levels.asks().len(), 1);
        assert_eq!(levels.asks()[0].price(), price(103));
        assert_eq!(levels.asks()[0].quantity(), qty(25));
    }
}

mod trades {
    use super::*;

    #[test]
    fn no_trades_when_no_match() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 110, 50)).unwrap();
        ob.match_orders();
        assert!(ob.trades().is_empty());
    }

    #[test]
    fn single_trade_on_exact_match() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();

        assert_eq!(ob.trades().len(), 1);
        let trade = ob.trades().last().unwrap();
        assert_eq!(trade.bid_trade.order_id(), order_id(1));
        assert_eq!(trade.ask_trade.order_id(), order_id(2));
        assert_eq!(trade.bid_trade.quantity(), qty(50));
    }

    #[test]
    fn trade_price_is_resting_order_price() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap(); // resting order
        ob.add(buy_order(2, 110, 50)).unwrap(); // aggressor willing to pay more
        ob.match_orders();

        let trade = ob.trades().last().unwrap();
        // Trade should execute at 100 (the resting ask price), not 110
        assert_eq!(trade.bid_trade.price(), price(100));
        assert_eq!(trade.ask_trade.price(), price(100));
    }

    #[test]
    fn multiple_trades_from_partial_fills() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 30)).unwrap();
        ob.add(sell_order(2, 100, 30)).unwrap();
        ob.add(buy_order(3, 100, 50)).unwrap();
        ob.match_orders();

        // First trade: buy 50 vs sell 30 -> fills 30
        // Second trade: buy remaining 20 vs sell 30 -> fills 20
        assert_eq!(ob.trades().len(), 2);

        let trades: Vec<_> = ob.trades().iter().collect();
        assert_eq!(trades[0].bid_trade.quantity(), qty(30));
        assert_eq!(trades[0].ask_trade.order_id(), order_id(1));
        assert_eq!(trades[1].bid_trade.quantity(), qty(20));
        assert_eq!(trades[1].ask_trade.order_id(), order_id(2));
    }

    #[test]
    fn trades_across_multiple_price_levels() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 10)).unwrap();
        ob.add(sell_order(2, 101, 10)).unwrap();
        ob.add(sell_order(3, 102, 10)).unwrap();
        ob.add(buy_order(4, 105, 25)).unwrap();
        ob.match_orders();

        assert_eq!(ob.trades().len(), 3);

        let trades: Vec<_> = ob.trades().iter().collect();
        assert_eq!(trades[0].bid_trade.price(), price(100));
        assert_eq!(trades[0].bid_trade.quantity(), qty(10));
        assert_eq!(trades[1].bid_trade.price(), price(101));
        assert_eq!(trades[1].bid_trade.quantity(), qty(10));
        assert_eq!(trades[2].bid_trade.price(), price(102));
        assert_eq!(trades[2].bid_trade.quantity(), qty(5));
    }

    #[test]
    fn trade_records_both_sides() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();

        let trade = ob.trades().last().unwrap();
        // Both sides should have same price and quantity
        assert_eq!(trade.bid_trade.price(), trade.ask_trade.price());
        assert_eq!(trade.bid_trade.quantity(), trade.ask_trade.quantity());
        // But different order IDs
        assert_ne!(trade.bid_trade.order_id(), trade.ask_trade.order_id());
    }

    #[test]
    fn trades_persist_after_multiple_match_calls() {
        let mut ob = Orderbook::new();

        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();
        assert_eq!(ob.trades().len(), 1);

        ob.add(buy_order(3, 100, 30)).unwrap();
        ob.add(sell_order(4, 100, 30)).unwrap();
        ob.match_orders();
        assert_eq!(ob.trades().len(), 2);
    }

    #[test]
    fn fak_trade_recorded_before_cancel() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 30)).unwrap();
        ob.add(buy_fak(2, 100, 50)).unwrap();
        ob.match_orders();

        // FAK partially filled then cancelled, but trade should be recorded
        assert_eq!(ob.trades().len(), 1);
        let trade = ob.trades().last().unwrap();
        assert_eq!(trade.bid_trade.quantity(), qty(30));
    }

    #[test]
    fn clear_trades_resets_history() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_order(2, 100, 50)).unwrap();
        ob.match_orders();
        assert_eq!(ob.trades().len(), 1);

        ob.clear_trades();
        assert!(ob.trades().is_empty());
    }
}

mod market_orders {
    use super::*;

    fn buy_market(id: u64, q: u32) -> Order {
        Order::new(order_id(id), OrderType::Market, Side::Buy, price(0), qty(q))
    }

    fn sell_market(id: u64, q: u32) -> Order {
        Order::new(
            order_id(id),
            OrderType::Market,
            Side::Sell,
            price(0),
            qty(q),
        )
    }

    #[test]
    fn market_buy_rejected_when_no_asks() {
        let mut ob = Orderbook::new();
        let result = ob.add(buy_market(1, 50));
        assert!(matches!(result, Err(OrderError::NoLiquidity)));
    }

    #[test]
    fn market_sell_rejected_when_no_bids() {
        let mut ob = Orderbook::new();
        let result = ob.add(sell_market(1, 50));
        assert!(matches!(result, Err(OrderError::NoLiquidity)));
    }

    #[test]
    fn market_buy_executes_at_ask_price() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 50)).unwrap();
        ob.add(buy_market(2, 50)).unwrap();
        ob.match_orders();

        let trade = ob.trades().last().unwrap();
        assert_eq!(trade.bid_trade.price(), price(100)); // executes at ask price, not MAX
    }

    #[test]
    fn market_sell_executes_at_bid_price() {
        let mut ob = Orderbook::new();
        ob.add(buy_order(1, 100, 50)).unwrap();
        ob.add(sell_market(2, 50)).unwrap();
        ob.match_orders();

        let trade = ob.trades().last().unwrap();
        assert_eq!(trade.ask_trade.price(), price(100)); // executes at bid price, not 0
    }

    #[test]
    fn market_order_sweeps_multiple_levels() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 10)).unwrap();
        ob.add(sell_order(2, 101, 10)).unwrap();
        ob.add(sell_order(3, 102, 10)).unwrap();
        ob.add(buy_market(4, 25)).unwrap();
        ob.match_orders();

        assert_eq!(ob.trades().len(), 3);
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks().len(), 1);
        assert_eq!(levels.asks()[0].quantity(), qty(5));
    }

    #[test]
    fn market_order_partial_fill_cancelled() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 30)).unwrap();
        ob.add(buy_market(2, 50)).unwrap();
        ob.match_orders();

        // Market order partially filled (30), remainder cancelled
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
        assert_eq!(ob.trades().len(), 1);
        assert_eq!(ob.trades().last().unwrap().bid_trade.quantity(), qty(30));
    }

    #[test]
    fn market_order_full_fill() {
        let mut ob = Orderbook::new();
        ob.add(sell_order(1, 100, 100)).unwrap();
        ob.add(buy_market(2, 50)).unwrap();
        ob.match_orders();

        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(50));
    }
}
