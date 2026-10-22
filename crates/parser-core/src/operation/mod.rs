pub mod swap;

use crate::operation::swap::{SwapInOperation, SwapOutOperation};

#[derive(PartialEq, Debug)]
pub enum Operation {
    SwapIn(SwapInOperation),
    SwapOut(SwapOutOperation)
}
