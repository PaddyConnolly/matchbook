#[derive(Debug, thiserror::Error)]
pub enum OrderError {
    #[error("Cannot fill order for more than the available quantity")]
    FillOverflow,
    #[error("Attempted to add order with existing order ID")]
    IdExists,
    #[error("Attempted to add order which couldn't be matched")]
    CantMatch,
    #[error("Order ID not found")]
    OrderNotFound,
    #[error("Not enough liquidity to fully fill order")]
    CantFullyFill,
    #[error("No liquidity to fill order")]
    NoLiquidity,
}
