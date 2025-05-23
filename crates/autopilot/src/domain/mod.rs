pub mod auction;
pub mod competition;
pub mod eth;
pub mod fee;
pub mod quote;
pub mod settlement;

pub use {
    auction::{
        Auction,
        RawAuctionData,
        order::{Order, OrderUid},
    },
    fee::ProtocolFees,
    quote::Quote,
};

#[derive(prometheus_metric_storage::MetricStorage)]
#[metric(subsystem = "domain")]
pub struct Metrics {
    /// How many times the solver marked as non-settling based on the database
    /// statistics.
    #[metric(labels("solver", "reason"))]
    pub banned_solver: prometheus::IntCounterVec,

    /// Tracks settlements that couldn't be matched to the database solutions.
    #[metric(labels("solver_address"))]
    pub inconsistent_settlements: prometheus::IntCounterVec,
}

impl Metrics {
    fn get() -> &'static Self {
        Metrics::instance(observe::metrics::get_storage_registry()).unwrap()
    }
}
