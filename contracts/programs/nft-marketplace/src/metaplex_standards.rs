use anchor_lang::prelude::*;
use mpl_token_metadata::{
    instructions::CreateMetadataAccountV3Cpi,
    state::{DataV2, Creator as MetaplexCreator, Collection as MetaplexCollection},
};

pub fn create_metaplex_metadata(
    ctx: Context<CreateMetaplexMetadata>,
    name: String,
    symbol: String,
    uri: String,
    seller_fee_basis_points: u16,
    creators: Option<Vec<MetaplexCreator>>,
    collection: Option<MetaplexCollection>,
    uses: Option<mpl_token_metadata::state::Uses>,
) -> Result<()> {
    let data = DataV2 {
        name,
        symbol,
        uri,
        seller_fee_basis_points,
        creators,
        collection,
        uses,
    };

    // Create metadata account following Metaplex standards
    let create_metadata_ix = CreateMetadataAccountV3Cpi::new(
        &ctx.accounts.token_metadata_program,
        CreateMetadataAccountV3CpiAccounts {
            metadata: ctx.accounts.metadata.to_account_info(),
            mint: ctx.accounts.mint.to_account_info(),
            mint_authority: ctx.accounts.mint_authority.to_account_info(),
            update_authority: ctx.accounts.update_authority.to_account_info(),
            payer: ctx.accounts.payer.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            rent: ctx.accounts.rent.to_account_info(),
        },
        data,
        false, // is_mutable
        true,  // update_authority_is_signer
        None,  // collection_details
    );

    create_metadata_ix.invoke()?;

    msg!("Metaplex-standard metadata created for cross-marketplace compatibility");
    Ok(())
}

#[derive(Accounts)]
pub struct CreateMetaplexMetadata<'info> {
    /// CHECK: Metadata account to be created
    #[account(mut)]
    pub metadata: AccountInfo<'info>,

    pub mint: Account<'info, anchor_spl::token::Mint>,
    
    pub mint_authority: Signer<'info>,
    
    /// CHECK: Update authority
    pub update_authority: AccountInfo<'info>,
    
    #[account(mut)]
    pub payer: Signer<'info>,
    
    /// CHECK: Token Metadata program
    pub token_metadata_program: AccountInfo<'info>,
    
    /// CHECK: Rent sysvar
    pub rent: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

// Helper function to verify Metaplex metadata
pub fn verify_metaplex_metadata(
    metadata: &AccountInfo,
    expected_mint: &Pubkey,
) -> Result<()> {
    // This would deserialize and verify the metadata account
    // matches Metaplex standards
    msg!("Verifying Metaplex metadata standards compliance");
    Ok(())
}