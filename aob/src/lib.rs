use anchor_lang::declare_id;

declare_id!("aaobKniTtDGvCZces7GH5UReLYP671bBkB96ahr9x3e");

#[doc(hidden)]
pub mod critbit;
#[doc(hidden)]
pub mod error;
pub mod orderbook;
pub mod params;
/// Describes the different data structres that the program uses to encode state
pub mod state;
pub mod utils;
