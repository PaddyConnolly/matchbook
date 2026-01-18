use matchbook::{Order, OrderId, OrderType, Orderbook, Price, Quantity, Side};

fn price(p: u64) -> Price {
    Price::new(p)
}

fn qty(q: u64) -> Quantity {
    Quantity(q)
}

fn order_id(id: &str) -> OrderId {
    OrderId::new(id.to_string())
}

fn buy_order(id: &str, p: u64, q: u64) -> Order {
    Order::new(
        order_id(id),
        OrderType::GoodTillCancelled,
        Side::Buy,
        price(p),
        qty(q),
    )
}

fn sell_order(id: &str, p: u64, q: u64) -> Order {
    Order::new(
        order_id(id),
        OrderType::GoodTillCancelled,
        Side::Sell,
        price(p),
        qty(q),
    )
}

fn buy_fak(id: &str, p: u64, q: u64) -> Order {
    Order::new(
        order_id(id),
        OrderType::FillAndKill,
        Side::Buy,
        price(p),
        qty(q),
    )
}

fn sell_fak(id: &str, p: u64, q: u64) -> Order {
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
        ob.add_order(buy_order("1", 100, 50)).unwrap();
        ob.add_order(sell_order("2", 110, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(50));
        assert_eq!(levels.asks()[0].quantity(), qty(50));
    }

    #[test]
    fn exact_match_removes_both_orders() {
        let mut ob = Orderbook::new();
        ob.add_order(buy_order("1", 100, 50)).unwrap();
        ob.add_order(sell_order("2", 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn partial_fill_bid_larger() {
        let mut ob = Orderbook::new();
        ob.add_order(buy_order("1", 100, 100)).unwrap();
        ob.add_order(sell_order("2", 100, 40)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(60));
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn partial_fill_ask_larger() {
        let mut ob = Orderbook::new();
        ob.add_order(buy_order("1", 100, 40)).unwrap();
        ob.add_order(sell_order("2", 100, 100)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(60));
    }

    #[test]
    fn match_when_bid_higher_than_ask() {
        let mut ob = Orderbook::new();
        ob.add_order(buy_order("1", 110, 50)).unwrap();
        ob.add_order(sell_order("2", 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn multiple_matches_in_sequence() {
        let mut ob = Orderbook::new();
        ob.add_order(sell_order("1", 100, 30)).unwrap();
        ob.add_order(sell_order("2", 100, 30)).unwrap();
        ob.add_order(buy_order("3", 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(10));
    }

    #[test]
    fn matches_best_price_first() {
        let mut ob = Orderbook::new();
        ob.add_order(sell_order("1", 90, 25)).unwrap();
        ob.add_order(sell_order("2", 100, 25)).unwrap();
        ob.add_order(buy_order("3", 100, 30)).unwrap();
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
        ob.add_order(sell_order("1", 100, 50)).unwrap();
        ob.add_order(sell_order("2", 100, 50)).unwrap();
        ob.add_order(buy_order("3", 100, 60)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert_eq!(levels.asks()[0].quantity(), qty(40));
    }

    #[test]
    fn match_clears_multiple_price_levels() {
        let mut ob = Orderbook::new();
        ob.add_order(sell_order("1", 100, 10)).unwrap();
        ob.add_order(sell_order("2", 101, 10)).unwrap();
        ob.add_order(sell_order("3", 102, 10)).unwrap();
        ob.add_order(buy_order("4", 105, 25)).unwrap();
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
        ob.add_order(sell_order("1", 100, 50)).unwrap();
        ob.add_order(buy_fak("2", 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn fak_partial_fill_remainder_cancelled() {
        let mut ob = Orderbook::new();
        ob.add_order(sell_order("1", 100, 30)).unwrap();
        ob.add_order(buy_fak("2", 100, 50)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert!(levels.bids().is_empty());
        assert!(levels.asks().is_empty());
    }

    #[test]
    fn fak_sell_works() {
        let mut ob = Orderbook::new();
        ob.add_order(buy_order("1", 100, 50)).unwrap();
        ob.add_order(sell_fak("2", 100, 30)).unwrap();
        ob.match_orders();
        let levels = ob.get_levels();
        assert_eq!(levels.bids()[0].quantity(), qty(20));
        assert!(levels.asks().is_empty());
    }
}

// ============== Market orders ==============

mod market_orders {
    use super::*;

    fn buy_market(id: &str, q: u64) -> Order {
        Order::new(order_id(id), OrderType::Market, Side::Buy, price(0), qty(q))
    }

    #[test]
    fn market_buy_executes_at_ask_price() {
        let mut ob = Orderbook::new();
        ob.add_order(sell_order("1", 100, 50)).unwrap();
        ob.add_order(buy_market("2", 50)).unwrap();
        ob.match_orders();
        let trade = ob.trades().last().unwrap();
        assert_eq!(trade.bid_trade.price(), price(100));
    }
}
