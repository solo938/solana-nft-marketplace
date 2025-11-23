use anchor_lang::prelude::*;
use mpl_bubblegum::{
    instructions::CreateTreeConfigCpi,
    state::TreeConfig,
};

#[account]
pub struct CompressionConfig {
    pub authority: Pubkey,
    pub merkle_tree: Pubkey,
    pub max_depth: u32,
    pub max_buffer_size: u32,
    pub bump: u8,
}

impl CompressionConfig {
    pub const LEN: usize = 32 + 32 + 4 + 4 + 1;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct CompressedNFTMetadata {
    pub name: String,
    pub symbol: String,
    pub uri: String,
    pub seller_fee_basis_points: u16,
    pub creators: Option<Vec<Creator>>,
    pub primary_sale_happened: bool,
    pub is_mutable: bool,
    pub edition_nonce: Option<u8>,
    pub token_standard: u8,
    pub collection: Option<Collection>,
    pub uses: Option<Uses>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Creator {
    pub address: Pubkey,
    pub verified: bool,
    pub share: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Collection {
    pub verified: bool,
    pub key: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct Uses {
    pub use_type: UseMethod,
    pub remaining: u64,
    pub total: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub enum UseMethod {
    Burn,
    Multiple,
    Single,
}