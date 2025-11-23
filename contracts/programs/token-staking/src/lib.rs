use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, TokenAccount, Transfer, Mint};


declare_id!("BBmtu4c4HtMLmDZhYCXWQUMwLBDfpUmZV2zeYn5W8AnA");

#[program]
pub mod token_staking {
    use super::*;

    pub fn initialize_pool(
        ctx: Context<InitializePool>,
        reward_rate: u64,
        lock_duration: i64,
    ) -> Result<()> {
        require!(reward_rate > 0, StakingError::InvalidRewardRate);
        require!(lock_duration >= 0, StakingError::InvalidLockDuration);

        let pool = &mut ctx.accounts.pool;
        pool.authority = ctx.accounts.authority.key();
        pool.staking_mint = ctx.accounts.staking_mint.key();
        pool.reward_mint = ctx.accounts.reward_mint.key();
        pool.reward_rate = reward_rate;
        pool.lock_duration = lock_duration;
        pool.total_staked = 0;
        pool.bump = ctx.bumps.pool;

        msg!("Staking pool initialized with reward rate: {}", reward_rate);
        Ok(())
    }

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::InvalidAmount);

        let pool = &mut ctx.accounts.pool;
        let user_stake = &mut ctx.accounts.user_stake;
        let clock = Clock::get()?;

        // Calculate pending rewards before updating stake
        if user_stake.amount > 0 {
            let pending_rewards = calculate_rewards(
                user_stake.amount,
                user_stake.last_update_time,
                clock.unix_timestamp,
                pool.reward_rate,
            )?;
            user_stake.pending_rewards += pending_rewards;
        }

        // Transfer tokens to pool
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_token_account.to_account_info(),
            to: ctx.accounts.pool_token_account.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        token::transfer(cpi_ctx, amount)?;

        // Update stake info
        user_stake.owner = ctx.accounts.user.key();
        user_stake.pool = pool.key();
        user_stake.amount += amount;
        user_stake.last_update_time = clock.unix_timestamp;
        user_stake.lock_end_time = clock.unix_timestamp + pool.lock_duration;

        pool.total_staked += amount;

        msg!("Staked {} tokens", amount);
        Ok(())
    }

    pub fn unstake(ctx: Context<Unstake>, amount: u64) -> Result<()> {
        require!(amount > 0, StakingError::InvalidAmount);
    
        let user_stake = &mut ctx.accounts.user_stake;
        let clock = Clock::get()?;
    
        require!(user_stake.amount >= amount, StakingError::InsufficientStake);
        require!(
            clock.unix_timestamp >= user_stake.lock_end_time,
            StakingError::StillLocked
        );
    
        // Get pool data before mutable borrow
        let pool_key = ctx.accounts.pool.key();
        let staking_mint = ctx.accounts.pool.staking_mint;
        let reward_rate = ctx.accounts.pool.reward_rate;
        let pool_bump = ctx.accounts.pool.bump;
    
        // Calculate and store pending rewards
        let pending_rewards = calculate_rewards(
            user_stake.amount,
            user_stake.last_update_time,
            clock.unix_timestamp,
            reward_rate,
        )?;
        user_stake.pending_rewards += pending_rewards;
    
        // Transfer tokens back to user
        let seeds = &[
            b"pool",
            staking_mint.as_ref(),
            &[pool_bump],
        ];
        let signer = &[&seeds[..]];
    
        let cpi_accounts = Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, amount)?;
    
        // Now update pool (mutable borrow)
        let pool = &mut ctx.accounts.pool;
        user_stake.amount -= amount;
        user_stake.last_update_time = clock.unix_timestamp;
        pool.total_staked -= amount;
    
        msg!("Unstaked {} tokens", amount);
        Ok(())
    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>) -> Result<()> {
        let user_stake = &mut ctx.accounts.user_stake;
        let clock = Clock::get()?;
    
        // Get pool data before any borrows
        let staking_mint = ctx.accounts.pool.staking_mint;
        let reward_rate = ctx.accounts.pool.reward_rate;
        let pool_bump = ctx.accounts.pool.bump;
    
        // Calculate total rewards
        let pending_rewards = calculate_rewards(
            user_stake.amount,
            user_stake.last_update_time,
            clock.unix_timestamp,
            reward_rate,
        )?;
        let total_rewards = user_stake.pending_rewards + pending_rewards;
    
        require!(total_rewards > 0, StakingError::NoRewardsToClaim);
    
        // Transfer rewards to user
        let seeds = &[
            b"pool",
            staking_mint.as_ref(),
            &[pool_bump],
        ];
        let signer = &[&seeds[..]];
    
        let cpi_accounts = Transfer {
            from: ctx.accounts.reward_vault.to_account_info(),
            to: ctx.accounts.user_reward_account.to_account_info(),
            authority: ctx.accounts.pool.to_account_info(),
        };
        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new_with_signer(cpi_program, cpi_accounts, signer);
        token::transfer(cpi_ctx, total_rewards)?;
    
        // Reset rewards
        user_stake.pending_rewards = 0;
        user_stake.last_update_time = clock.unix_timestamp;
        user_stake.total_claimed += total_rewards;
    
        msg!("Claimed {} reward tokens", total_rewards);
        Ok(())
    }


    pub fn update_reward_rate(
        ctx: Context<UpdateRewardRate>,
        new_rate: u64,
    ) -> Result<()> {
        require!(new_rate > 0, StakingError::InvalidRewardRate);

        let pool = &mut ctx.accounts.pool;
        pool.reward_rate = new_rate;

        msg!("Reward rate updated to {}", new_rate);
        Ok(())
    }
}

fn calculate_rewards(
    staked_amount: u64,
    last_update: i64,
    current_time: i64,
    reward_rate: u64,
) -> Result<u64> {
    let time_elapsed = (current_time - last_update) as u64;
    let rewards = (staked_amount as u128)
        .checked_mul(reward_rate as u128)
        .unwrap()
        .checked_mul(time_elapsed as u128)
        .unwrap()
        .checked_div(86400) // Daily rate
        .unwrap()
        .checked_div(1_000_000) // Precision adjustment
        .unwrap() as u64;
    Ok(rewards)
}

#[derive(Accounts)]
pub struct InitializePool<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + StakingPool::LEN,
        seeds = [b"pool", staking_mint.key().as_ref()],
        bump
    )]
    pub pool: Account<'info, StakingPool>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub staking_mint: Account<'info, Mint>,
    pub reward_mint: Account<'info, Mint>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Stake<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.staking_mint.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakingPool>,

    #[account(
        init,
        payer = user,
        space = 8 + UserStake::LEN,
        seeds = [b"user_stake", pool.key().as_ref(), user.key().as_ref()],
        bump
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_token_account.mint == pool.staking_mint,
        constraint = user_token_account.owner == user.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = pool_token_account.mint == pool.staking_mint
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Unstake<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.staking_mint.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakingPool>,

    #[account(
        mut,
        seeds = [b"user_stake", pool.key().as_ref(), user.key().as_ref()],
        bump,
        constraint = user_stake.owner == user.key()
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = user_token_account.mint == pool.staking_mint,
        constraint = user_token_account.owner == user.key()
    )]
    pub user_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = pool_token_account.mint == pool.staking_mint
    )]
    pub pool_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(
        seeds = [b"pool", pool.staking_mint.as_ref()],
        bump = pool.bump
    )]
    pub pool: Account<'info, StakingPool>,

    #[account(
        mut,
        seeds = [b"user_stake", pool.key().as_ref(), user.key().as_ref()],
        bump,
        constraint = user_stake.owner == user.key()
    )]
    pub user_stake: Account<'info, UserStake>,

    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        constraint = reward_vault.mint == pool.reward_mint
    )]
    pub reward_vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        constraint = user_reward_account.mint == pool.reward_mint,
        constraint = user_reward_account.owner == user.key()
    )]
    pub user_reward_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct UpdateRewardRate<'info> {
    #[account(
        mut,
        seeds = [b"pool", pool.staking_mint.as_ref()],
        bump = pool.bump,
        constraint = pool.authority == authority.key()
    )]
    pub pool: Account<'info, StakingPool>,

    pub authority: Signer<'info>,
}

#[account]
pub struct StakingPool {
    pub authority: Pubkey,
    pub staking_mint: Pubkey,
    pub reward_mint: Pubkey,
    pub reward_rate: u64,
    pub lock_duration: i64,
    pub total_staked: u64,
    pub bump: u8,
}

impl StakingPool {
    pub const LEN: usize = 32 + 32 + 32 + 8 + 8 + 8 + 1;
}

#[account]
pub struct UserStake {
    pub owner: Pubkey,
    pub pool: Pubkey,
    pub amount: u64,
    pub pending_rewards: u64,
    pub last_update_time: i64,
    pub lock_end_time: i64,
    pub total_claimed: u64,
}

impl UserStake {
    pub const LEN: usize = 32 + 32 + 8 + 8 + 8 + 8 + 8;
}

#[error_code]
pub enum StakingError {
    #[msg("Invalid reward rate")]
    InvalidRewardRate,
    #[msg("Invalid lock duration")]
    InvalidLockDuration,
    #[msg("Invalid amount")]
    InvalidAmount,
    #[msg("Insufficient stake")]
    InsufficientStake,
    #[msg("Tokens are still locked")]
    StillLocked,
    #[msg("No rewards to claim")]
    NoRewardsToClaim,
}
