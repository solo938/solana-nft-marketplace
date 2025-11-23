use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use mpl_token_metadata::state::{Metadata, TokenMetadataAccount};

pub mod compression;
pub mod compressed_nft;
pub mod metaplex_standards;

use compression::*;
use compressed_nft::*;
use metaplex_standards::*;




declare_id!("42t5wRBjbmH44vgNCxuL9Mpyqg3qDWReDz3UnsGyG6VG");

#[program]
pub mod nft_marketplace {
    use super::*;

    pub fn initialize_marketplace(
        ctx: Context<InitializeMarketplace>,
        fee_basis_points: u16,
    ) -> Result<()> {
        require!(fee_basis_points <= 10000, MarketplaceError::InvalidFee);
        
        let marketplace = &mut ctx.accounts.marketplace;
        marketplace.authority = ctx.accounts.authority.key();
        marketplace.fee_basis_points = fee_basis_points;
        marketplace.treasury = ctx.accounts.treasury.key();
        marketplace.total_sales = 0;
        marketplace.total_volume = 0;
        marketplace.bump = ctx.bumps.marketplace;
        
        msg!("Marketplace initialized with {}% fee", fee_basis_points as f64 / 100.0);
        Ok(())
    }

    pub fn list_nft(
        ctx: Context<ListNFT>,
        price: u64,
        royalty_percentage: u16,
    ) -> Result<()> {
        require!(price > 0, MarketplaceError::InvalidPrice);
        require!(royalty_percentage <= 5000, MarketplaceError::InvalidRoyalty);

        let listing = &mut ctx.accounts.listing;
        listing.seller = ctx.accounts.seller.key();
        listing.nft_mint = ctx.accounts.nft_mint.key();
        listing.price = price;
        listing.royalty_percentage = royalty_percentage;
        listing.royalty_recipient = ctx.accounts.seller.key();
        listing.is_active = true;
        listing.listed_at = Clock::get()?.unix_timestamp;
        listing.bump = ctx.bumps.listing;

        // Transfer NFT to escrow
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_nft_account.to_account_info(),
            to: ctx.accounts.escrow_nft_account.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, 1)?;

        msg!("NFT listed for {} lamports", price);
        Ok(())
    }

    }

    pub fn buy_nft(ctx: Context<BuyNFT>) -> Result<()> {
        let listing = &mut ctx.accounts.listing;
        require!(listing.is_active, MarketplaceError::ListingNotActive);
    
        let price = listing.price;
        let marketplace = &ctx.accounts.marketplace;
        
        // Calculate fees
        let marketplace_fee = (price as u128)
            .checked_mul(marketplace.fee_basis_points as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        
        let royalty_fee = (price as u128)
            .checked_mul(listing.royalty_percentage as u128)
            .unwrap()
            .checked_div(10000)
            .unwrap() as u64;
        
        let seller_amount = price
            .checked_sub(marketplace_fee)
            .unwrap()
            .checked_sub(royalty_fee)
            .unwrap();
    
        // NEW: Verify Metaplex metadata standards for cross-marketplace compatibility
        verify_metaplex_metadata(
            &ctx.accounts.nft_metadata,
            &listing.nft_mint,
        )?;
    
        // Transfer marketplace fee to treasury
        **ctx.accounts.buyer.to_account_info().try_borrow_mut_lamports()? -= marketplace_fee;
        **ctx.accounts.treasury.to_account_info().try_borrow_mut_lamports()? += marketplace_fee;
    
        // Transfer royalty to creator
        **ctx.accounts.buyer.to_account_info().try_borrow_mut_lamports()? -= royalty_fee;
        **ctx.accounts.royalty_recipient.to_account_info().try_borrow_mut_lamports()? += royalty_fee;
    
        // Transfer remaining to seller
        **ctx.accounts.buyer.to_account_info().try_borrow_mut_lamports()? -= seller_amount;
        **ctx.accounts.seller.to_account_info().try_borrow_mut_lamports()? += seller_amount;
    
        // NEW: Check if this is a compressed NFT or standard NFT
        if ctx.accounts.is_compressed_nft.is_some() {
            // Handle compressed NFT transfer using Bubblegum
            msg!("Processing compressed NFT transfer...");
            // Compressed NFTs use different transfer logic
            // In practice, you'd use Bubblegum's transfer instructions
        } else {
            // Standard NFT transfer (your existing logic)
            let seeds = &[
                b"listing",
                listing.nft_mint.as_ref(),
                &[listing.bump],
            ];
            let signer = &[&seeds[..]];
    
            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_nft_account.to_account_info(),
                to: ctx.accounts.buyer_nft_account.to_account_info(),
                authority: ctx.accounts.listing.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, 1)?;
        }
    
        // Update listing status
        listing.is_active = false;
    
        // Update marketplace stats
        let marketplace = &mut ctx.accounts.marketplace;
        marketplace.total_sales += 1;
        marketplace.total_volume += price;
    
        msg!("NFT sold for {} lamports with Metaplex standards verification", price);
        Ok(())
    }

    pub fn create_auction(
        ctx: Context<CreateAuction>,
        starting_price: u64,
        reserve_price: u64,
        duration: i64,
    ) -> Result<()> {
        require!(starting_price > 0, MarketplaceError::InvalidPrice);
        require!(reserve_price >= starting_price, MarketplaceError::InvalidReservePrice);
        require!(duration > 0, MarketplaceError::InvalidDuration);

        let auction = &mut ctx.accounts.auction;
        auction.seller = ctx.accounts.seller.key();
        auction.nft_mint = ctx.accounts.nft_mint.key();
        auction.starting_price = starting_price;
        auction.current_bid = 0;
        auction.reserve_price = reserve_price;
        auction.highest_bidder = None;
        auction.start_time = Clock::get()?.unix_timestamp;
        auction.end_time = auction.start_time + duration;
        auction.is_active = true;
        auction.bump = ctx.bumps.auction;

        // Transfer NFT to escrow
        let cpi_accounts = Transfer {
            from: ctx.accounts.seller_nft_account.to_account_info(),
            to: ctx.accounts.escrow_nft_account.to_account_info(),
            authority: ctx.accounts.seller.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, 1)?;

        msg!("Auction created with starting price {} lamports", starting_price);
        Ok(())
    }

    pub fn place_bid(ctx: Context<PlaceBid>, bid_amount: u64) -> Result<()> {
        let auction = &mut ctx.accounts.auction;
        let clock = Clock::get()?;
        
        require!(auction.is_active, MarketplaceError::AuctionNotActive);
        require!(clock.unix_timestamp < auction.end_time, MarketplaceError::AuctionEnded);
        require!(bid_amount > auction.current_bid, MarketplaceError::BidTooLow);
        require!(bid_amount >= auction.starting_price, MarketplaceError::BidBelowStarting);

        // Refund previous bidder if exists
        if let Some(previous_bidder) = auction.highest_bidder {
            **ctx.accounts.auction.to_account_info().try_borrow_mut_lamports()? -= auction.current_bid;
            **ctx.accounts.previous_bidder.to_account_info().try_borrow_mut_lamports()? += auction.current_bid;
        }

        // Escrow new bid
        **ctx.accounts.bidder.to_account_info().try_borrow_mut_lamports()? -= bid_amount;
        **ctx.accounts.auction.to_account_info().try_borrow_mut_lamports()? += bid_amount;

        auction.current_bid = bid_amount;
        auction.highest_bidder = Some(ctx.accounts.bidder.key());

        msg!("Bid placed for {} lamports", bid_amount);
        Ok(())
    }

    pub fn settle_auction(ctx: Context<SettleAuction>) -> Result<()> {
        let auction = &mut ctx.accounts.auction;
        let clock = Clock::get()?;
        
        require!(auction.is_active, MarketplaceError::AuctionNotActive);
        require!(clock.unix_timestamp >= auction.end_time, MarketplaceError::AuctionNotEnded);

        if auction.current_bid >= auction.reserve_price && auction.highest_bidder.is_some() {
            let marketplace = &ctx.accounts.marketplace;
            let price = auction.current_bid;
            
            // Calculate fees
            let marketplace_fee = (price as u128)
                .checked_mul(marketplace.fee_basis_points as u128)
                .unwrap()
                .checked_div(10000)
                .unwrap() as u64;
            
            let seller_amount = price.checked_sub(marketplace_fee).unwrap();

            // Transfer fees and payment
            **ctx.accounts.auction.to_account_info().try_borrow_mut_lamports()? -= marketplace_fee;
            **ctx.accounts.treasury.to_account_info().try_borrow_mut_lamports()? += marketplace_fee;

            **ctx.accounts.auction.to_account_info().try_borrow_mut_lamports()? -= seller_amount;
            **ctx.accounts.seller.to_account_info().try_borrow_mut_lamports()? += seller_amount;

            // Transfer NFT to winner
            let seeds = &[
                b"auction",
                auction.nft_mint.as_ref(),
                &[auction.bump],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_nft_account.to_account_info(),
                to: ctx.accounts.winner_nft_account.to_account_info(),
                authority: ctx.accounts.auction.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, 1)?;

            msg!("Auction settled - NFT sold for {} lamports", price);
        } else {
            // Return NFT to seller if reserve not met
            let seeds = &[
                b"auction",
                auction.nft_mint.as_ref(),
                &[auction.bump],
            ];
            let signer = &[&seeds[..]];

            let cpi_accounts = Transfer {
                from: ctx.accounts.escrow_nft_account.to_account_info(),
                to: ctx.accounts.seller_nft_account.to_account_info(),
                authority: ctx.accounts.auction.to_account_info(),
            };
            let cpi_program = ctx.accounts.token_program.to_account_info();
            let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
            token::transfer(cpi_ctx, 1)?;

            // Refund highest bidder
            if auction.highest_bidder.is_some() {
                **ctx.accounts.auction.to_account_info().try_borrow_mut_lamports()? -= auction.current_bid;
                **ctx.accounts.winner.to_account_info().try_borrow_mut_lamports()? += auction.current_bid;
            }

            msg!("Auction ended - reserve price not met");
        }

        auction.is_active = false;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeMarketplace<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Marketplace::LEN,
        seeds = [b"marketplace"],
        bump
    )]
    pub marketplace: Account<'info, Marketplace>,
    
    #[account(mut)]
    pub authority: Signer<'info>,
    
    /// CHECK: Treasury account for marketplace fees
    pub treasury: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct ListNFT<'info> {
    #[account(
        init,
        payer = seller,
        space = 8 + Listing::LEN,
        seeds = [b"listing", nft_mint.key().as_ref()],
        bump
    )]
    pub listing: Account<'info, Listing>,
    
    #[account(mut)]
    pub seller: Signer<'info>,
    
    pub nft_mint: Account<'info, token::Mint>,
    
    #[account(
        mut,
        constraint = seller_nft_account.mint == nft_mint.key(),
        constraint = seller_nft_account.owner == seller.key(),
        constraint = seller_nft_account.amount == 1
    )]
    pub seller_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = seller,
        associated_token::mint = nft_mint,
        associated_token::authority = listing
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct BuyNFT<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump = marketplace.bump
    )]
    pub marketplace: Account<'info, Marketplace>,
    
    #[account(
        mut,
        seeds = [b"listing", listing.nft_mint.as_ref()],
        bump = listing.bump,
        constraint = listing.is_active
    )]
    pub listing: Account<'info, Listing>,
    
    #[account(mut)]
    pub buyer: Signer<'info>,
    
    #[account(mut)]
    pub seller: SystemAccount<'info>,
    
    #[account(
        mut,
        constraint = escrow_nft_account.mint == listing.nft_mint,
        constraint = escrow_nft_account.amount == 1
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = buyer,
        associated_token::mint = listing.nft_mint,
        associated_token::authority = buyer
    )]
    pub buyer_nft_account: Account<'info, TokenAccount>,
    
    /// CHECK: Treasury account
    #[account(mut, constraint = treasury.key() == marketplace.treasury)]
    pub treasury: AccountInfo<'info>,
    
    /// CHECK: Royalty recipient
    #[account(mut)]
    pub royalty_recipient: AccountInfo<'info>,
    
    // NEW: Metaplex metadata account for standards verification
    /// CHECK: Metaplex metadata account
    pub nft_metadata: AccountInfo<'info>,
    
    // NEW: Optional flag for compressed NFTs
    /// CHECK: Optional - indicates if this is a compressed NFT
    pub is_compressed_nft: Option<AccountInfo<'info>>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CancelListing<'info> {
    #[account(
        mut,
        seeds = [b"listing", listing.nft_mint.as_ref()],
        bump = listing.bump,
        constraint = listing.seller == seller.key(),
        constraint = listing.is_active
    )]
    pub listing: Account<'info, Listing>,
    
    #[account(mut)]
    pub seller: Signer<'info>,
    
    #[account(
        mut,
        constraint = escrow_nft_account.mint == listing.nft_mint,
        constraint = escrow_nft_account.amount == 1
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = seller_nft_account.mint == listing.nft_mint,
        constraint = seller_nft_account.owner == seller.key()
    )]
    pub seller_nft_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct CreateAuction<'info> {
    #[account(
        init,
        payer = seller,
        space = 8 + Auction::LEN,
        seeds = [b"auction", nft_mint.key().as_ref()],
        bump
    )]
    pub auction: Account<'info, Auction>,
    
    #[account(mut)]
    pub seller: Signer<'info>,
    
    pub nft_mint: Account<'info, token::Mint>,
    
    #[account(
        mut,
        constraint = seller_nft_account.mint == nft_mint.key(),
        constraint = seller_nft_account.owner == seller.key(),
        constraint = seller_nft_account.amount == 1
    )]
    pub seller_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = seller,
        associated_token::mint = nft_mint,
        associated_token::authority = auction
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PlaceBid<'info> {
    #[account(
        mut,
        seeds = [b"auction", auction.nft_mint.as_ref()],
        bump = auction.bump
    )]
    pub auction: Account<'info, Auction>,
    
    #[account(mut)]
    pub bidder: Signer<'info>,
    
    /// CHECK: Previous bidder (optional)
    #[account(mut)]
    pub previous_bidder: AccountInfo<'info>,
    
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct SettleAuction<'info> {
    #[account(
        mut,
        seeds = [b"marketplace"],
        bump = marketplace.bump
    )]
    pub marketplace: Account<'info, Marketplace>,
    
    #[account(
        mut,
        seeds = [b"auction", auction.nft_mint.as_ref()],
        bump = auction.bump
    )]
    pub auction: Account<'info, Auction>,
    
    #[account(mut)]
    pub seller: SystemAccount<'info>,
    
    /// CHECK: Winner (highest bidder)
    #[account(mut)]
    pub winner: AccountInfo<'info>,
    
    #[account(
        mut,
        constraint = escrow_nft_account.mint == auction.nft_mint
    )]
    pub escrow_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        init_if_needed,
        payer = winner,
        associated_token::mint = auction.nft_mint,
        associated_token::authority = winner
    )]
    pub winner_nft_account: Account<'info, TokenAccount>,
    
    #[account(
        mut,
        constraint = seller_nft_account.mint == auction.nft_mint
    )]
    pub seller_nft_account: Account<'info, TokenAccount>,
    
    /// CHECK: Treasury account
    #[account(mut, constraint = treasury.key() == marketplace.treasury)]
    pub treasury: AccountInfo<'info>,
    
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Marketplace {
    pub authority: Pubkey,
    pub treasury: Pubkey,
    pub fee_basis_points: u16,
    pub total_sales: u64,
    pub total_volume: u64,
    pub bump: u8,
}

impl Marketplace {
    pub const LEN: usize = 32 + 32 + 2 + 8 + 8 + 1;
}

#[account]
pub struct Listing {
    pub seller: Pubkey,
    pub nft_mint: Pubkey,
    pub price: u64,
    pub royalty_percentage: u16,
    pub royalty_recipient: Pubkey,
    pub is_active: bool,
    pub listed_at: i64,
    pub bump: u8,
}

impl Listing {
    pub const LEN: usize = 32 + 32 + 8 + 2 + 32 + 1 + 8 + 1;
}

#[account]
pub struct Auction {
    pub seller: Pubkey,
    pub nft_mint: Pubkey,
    pub starting_price: u64,
    pub current_bid: u64,
    pub reserve_price: u64,
    pub highest_bidder: Option<Pubkey>,
    pub start_time: i64,
    pub end_time: i64,
    pub is_active: bool,
    pub bump: u8,
}

impl Auction {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + (1 + 32) + 8 + 8 + 1 + 1;
}

#[error_code]
pub enum MarketplaceError {
    #[msg("Invalid fee percentage")]
    InvalidFee,
    #[msg("Invalid price")]
    InvalidPrice,
    #[msg("Invalid royalty percentage")]
    InvalidRoyalty,
    #[msg("Listing is not active")]
    ListingNotActive,
    #[msg("Auction is not active")]
    AuctionNotActive,
    #[msg("Auction has ended")]
    AuctionEnded,
    #[msg("Auction has not ended yet")]
    AuctionNotEnded,
    #[msg("Bid amount is too low")]
    BidTooLow,
    #[msg("Bid is below starting price")]
    BidBelowStarting,
    #[msg("Invalid reserve price")]
    InvalidReservePrice,
    #[msg("Invalid duration")]
    InvalidDuration,
    // NEW ERRORS:
    #[msg("NFT metadata does not comply with Metaplex standards")]
    InvalidMetadata,
    #[msg("Invalid token standard")]
    InvalidTokenStandard,
    #[msg("Compressed NFT transfer failed")]
    CompressedNFTTransferFailed,
}