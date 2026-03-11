//! Route discovery per trumpet layer.
//! L1: triangular arb (A→B→C→A), L2: quad-leg, L3: hedge handoff, L4: EQ smoothing.

use fsr_resonance::compute_net_edge;
use fsr_types::{
    ids::{RouteId, TradingPair, VenueId},
    market::{OrderBook, OrderSide},
    artifacts::{RouteCandidate, RouteLeg, TrumpetLayer},
    Q32,
};
use fsr_fixed::{q32_from_ratio, ONE};

/// Synthetic route discovery from order books.
/// Generates candidate routes for a given trumpet layer.
pub fn discover_routes(
    layer: TrumpetLayer,
    books: &[OrderBook],
    route_id_start: u64,
    fee_bp: i64,
    slip_bp: i64,
    tau_edge: Q32,
) -> Vec<RouteCandidate> {
    match layer {
        TrumpetLayer::L1 => discover_l1_triangular(books, route_id_start, fee_bp, slip_bp, tau_edge),
        TrumpetLayer::L2 => discover_l2_quad(books, route_id_start, fee_bp, slip_bp, tau_edge),
        TrumpetLayer::L3 => discover_l3_hedge(books, route_id_start, fee_bp, slip_bp, tau_edge),
        TrumpetLayer::L4 => discover_l4_eq(books, route_id_start, fee_bp, slip_bp, tau_edge),
    }
}

/// L1: Triangular arbitrage (A→B→C→A) using synthetic book data.
fn discover_l1_triangular(
    books: &[OrderBook],
    id_start: u64,
    fee_bp: i64,
    slip_bp: i64,
    tau_edge: Q32,
) -> Vec<RouteCandidate> {
    let mut results = Vec::new();
    let mut id = id_start;

    // Try all triplets of books
    for i in 0..books.len().min(4) {
        for j in 0..books.len().min(4) {
            if j == i { continue; }
            for k in 0..books.len().min(4) {
                if k == i || k == j { continue; }
                let b_i = &books[i];
                let b_j = &books[j];
                let b_k = &books[k];
                if let (Some(p1), Some(p2), Some(p3)) = (b_i.best_ask_bp(), b_j.best_ask_bp(), b_k.best_bid_bp()) {
                    // Simple price product: buy i, buy j, sell k
                    let price1 = q32_from_ratio(p1, 10_000); // as Q32 ratio
                    let price2 = q32_from_ratio(p2, 10_000);
                    let price3 = q32_from_ratio(p3, 10_000);
                    let cost1 = (fee_bp + slip_bp) * ONE / 10_000;
                    let cost2 = cost1;
                    let cost3 = cost1;
                    let net = compute_net_edge(&[price1, price2, price3], &[cost1, cost2, cost3]);
                    if net >= tau_edge {
                        results.push(RouteCandidate {
                            route_id: RouteId(id),
                            layer: TrumpetLayer::L1,
                            si_score: net,
                            net_edge: net,
                            legs: vec![
                                make_leg(b_i.venue.clone(), b_i.pair.clone(), OrderSide::Buy, 1, p1, fee_bp, slip_bp),
                                make_leg(b_j.venue.clone(), b_j.pair.clone(), OrderSide::Buy, 1, p2, fee_bp, slip_bp),
                                make_leg(b_k.venue.clone(), b_k.pair.clone(), OrderSide::Sell, 1, p3, fee_bp, slip_bp),
                            ],
                        });
                        id += 1;
                    }
                }
            }
        }
    }
    results
}

/// L2: Quad-leg arbitrage (max_cycle_len = 4).
fn discover_l2_quad(
    books: &[OrderBook],
    id_start: u64,
    fee_bp: i64,
    slip_bp: i64,
    tau_edge: Q32,
) -> Vec<RouteCandidate> {
    // Simplified: generate a single synthetic quad-leg candidate if books available
    if books.len() < 2 {
        return vec![];
    }
    let mut results = Vec::new();
    let b0 = &books[0];
    let b1 = &books[books.len() - 1];
    if let (Some(ask0), Some(bid1)) = (b0.best_ask_bp(), b1.best_bid_bp()) {
        let p0 = q32_from_ratio(ask0, 10_000);
        let p1 = q32_from_ratio(bid1, 10_000);
        let cost = (fee_bp + slip_bp) * ONE / 10_000;
        let net = compute_net_edge(&[p0, ONE, p1, ONE], &[cost, cost, cost, cost]);
        if net >= tau_edge {
            results.push(RouteCandidate {
                route_id: RouteId(id_start),
                layer: TrumpetLayer::L2,
                si_score: net,
                net_edge: net,
                legs: vec![
                    make_leg(b0.venue.clone(), b0.pair.clone(), OrderSide::Buy, 1, ask0, fee_bp, slip_bp),
                    make_leg(b1.venue.clone(), b1.pair.clone(), OrderSide::Sell, 1, bid1, fee_bp, slip_bp),
                ],
            });
        }
    }
    results
}

/// L3: Hedge handoff routes.
fn discover_l3_hedge(
    books: &[OrderBook],
    id_start: u64,
    fee_bp: i64,
    slip_bp: i64,
    _tau_edge: Q32,
) -> Vec<RouteCandidate> {
    // L3 produces hedge handoff routes (simplified: one per available book pair)
    if books.is_empty() {
        return vec![];
    }
    let b = &books[0];
    let mut results = Vec::new();
    if let Some(mid) = b.mid_bp() {
        results.push(RouteCandidate {
            route_id: RouteId(id_start),
            layer: TrumpetLayer::L3,
            si_score: 0,
            net_edge: -fee_bp * ONE / 10_000,
            legs: vec![
                make_leg(b.venue.clone(), b.pair.clone(), OrderSide::Sell, 1, mid, fee_bp, slip_bp),
            ],
        });
    }
    results
}

/// L4: EQ smoothing (inventory imbalance routes, no speculative intent).
fn discover_l4_eq(
    books: &[OrderBook],
    id_start: u64,
    fee_bp: i64,
    slip_bp: i64,
    _tau_edge: Q32,
) -> Vec<RouteCandidate> {
    if books.is_empty() {
        return vec![];
    }
    let b = &books[0];
    let mut results = Vec::new();
    if let Some(mid) = b.mid_bp() {
        results.push(RouteCandidate {
            route_id: RouteId(id_start + 1000),
            layer: TrumpetLayer::L4,
            si_score: 0,
            net_edge: 0,
            legs: vec![
                make_leg(b.venue.clone(), b.pair.clone(), OrderSide::Buy, 1, mid, fee_bp, slip_bp),
            ],
        });
    }
    results
}

fn make_leg(
    venue: VenueId,
    pair: TradingPair,
    side: OrderSide,
    qty: u64,
    price_bp: i64,
    fee_bp: i64,
    slip_bp: i64,
) -> RouteLeg {
    RouteLeg {
        venue,
        pair,
        side,
        quantity_lots: qty,
        price_bp,
        fee_bp,
        slip_bp,
    }
}
