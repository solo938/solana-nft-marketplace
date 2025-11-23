use anchor_lang::prelude::*;
use mpl_bubblegum::{
    instructions::MintV1Cpi,
    state::MetadataArgs,
};
use crate::compression::{CompressedNFTMetadata, CompressionConfig};

pub fn mint_compressed_nft(
    ctx: Context<MintCompressedNFT>,
    metadata: CompressedNFTMetadata,
) -> Result<()> {
    let compression_config = &ctx.accounts.compression_config;
    
    // Convert to Bubblegum metadata format
    let metadata_args = MetadataArgs {
        name: metadata.name,
        symbol: metadata.symbol,
        uri: metadata.uri,
        seller_fee_basis_points: metadata.seller_fee_basis_points,
        primary_sale_happened: metadata.primary_sale_happened,
        is_mutable: metadata.is_mutable,
        edition_nonce: metadata.edition_nonce,
        token_standard: Some(mpl_token_metadata::state::TokenStandard::NonFungible),
        collection: metadata.collection.map(|c| mpl_bubblegum::state::Collection {
            verified: c.verified,
            key: c.key,
        }),
        uses: metadata.uses.map(|u| mpl_bubblegum::state::Uses {
            use_method: match u.use_type {
                crate::compression::UseMethod::Burn => mpl_bubblegum::state::UseMethod::Burn,
                crate::compression::UseMethod::Multiple => mpl_bubblegum::state::UseMethod::Multiple,
                crate::compression::UseMethod::Single => mpl_bubblegum::state::UseMethod::Single,
            },
            remaining: u.remaining,
            total: u.total,
        }),
        creators: metadata.creators.map(|creators| {
            creators.into_iter().map(|c| mpl_bubblegum::state::Creator {
                address: c.address,
                verified: c.verified,
                share: c.share,
            }).collect()
        }),
        token_program_version: mpl_bubblegum::state::TokenProgramVersion::Original,
    };

    // Mint compressed NFT using Bubblegum
    let mint_ix = MintV1Cpi::new(
        &ctx.accounts.bubblegum_program,
        MintV1CpiAccounts {
            tree_config: ctx.accounts.tree_config.to_account_info(),
            leaf_owner: ctx.accounts.recipient.to_account_info(),
            leaf_delegate: ctx.accounts.recipient.to_account_info(),
            merkle_tree: ctx.accounts.merkle_tree.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            tree_creator_or_delegate: ctx.accounts.authority.to_account_info(),
            log_wrapper: ctx.accounts.log_wrapper.to_account_info(),
            compression_program: ctx.accounts.compression_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        },
        metadata_args,
    );

    mint_ix.invoke()?;

    msg!("Compressed NFT minted successfully with 1000x cheaper storage");
    Ok(())
}

#[derive(Accounts)]
pub struct MintCompressedNFT<'info> {
    #[account(
        seeds = [b"compression"],
        bump = compression_config.bump
    )]
    pub compression_config: Account<'info, CompressionConfig>,

    #[account(mut)]
    pub authority: Signer<'info>,

    /// CHECK: Recipient of the compressed NFT
    pub recipient: AccountInfo<'info>,

    /// CHECK: Merkle tree account
    #[account(mut)]
    pub merkle_tree: AccountInfo<'info>,

    /// CHECK: Tree config account
    #[account(mut)]
    pub tree_config: AccountInfo<'info>,

    #[account(mut)]
    pub payer: Signer<'info>,

    /// CHECK: Bubblegum program
    pub bubblegum_program: AccountInfo<'info>,

    /// CHECK: Compression program
    pub compression_program: AccountInfo<'info>,

    /// CHECK: Log wrapper program
    pub log_wrapper: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}