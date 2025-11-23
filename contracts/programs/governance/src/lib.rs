use anchor_lang::prelude::*;

declare_id!("AdapsPPd59DBL6e73spfef2iGdhdX6CLmn9jborntxrZ");

#[program]
pub mod governance {
    use super::*;

    pub fn initialize_dao(
        ctx: Context<InitializeDao>,
        voting_period: i64,
        quorum_percentage: u8,
        approval_threshold: u8,
    ) -> Result<()> {
        require!(voting_period > 0, GovernanceError::InvalidVotingPeriod);
        require!(quorum_percentage <= 100, GovernanceError::InvalidQuorum);
        require!(approval_threshold <= 100, GovernanceError::InvalidThreshold);

        let dao = &mut ctx.accounts.dao;
        dao.authority = ctx.accounts.authority.key();
        dao.voting_period = voting_period;
        dao.quorum_percentage = quorum_percentage;
        dao.approval_threshold = approval_threshold;
        dao.proposal_count = 0;
        dao.bump = ctx.bumps.dao;

        msg!("DAO initialized with {}% quorum", quorum_percentage);
        Ok(())
    }

    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        title: String,
        description: String,
        proposal_type: ProposalType,
    ) -> Result<()> {
        require!(title.len() <= 100, GovernanceError::TitleTooLong);
        require!(description.len() <= 500, GovernanceError::DescriptionTooLong);

        let dao = &mut ctx.accounts.dao;
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;

        proposal.dao = dao.key();
        proposal.proposer = ctx.accounts.proposer.key();
        proposal.title = title;
        proposal.description = description;
        proposal.proposal_type = proposal_type;
        proposal.votes_for = 0;
        proposal.votes_against = 0;
        proposal.status = ProposalStatus::Active;
        proposal.start_time = clock.unix_timestamp;
        proposal.end_time = clock.unix_timestamp + dao.voting_period;
        proposal.executed = false;
        proposal.bump = ctx.bumps.proposal;

        dao.proposal_count += 1;

        msg!("Proposal created: {}", proposal.title);
        Ok(())
    }

    pub fn cast_vote(
        ctx: Context<CastVote>,
        vote: Vote,
        weight: u64,
    ) -> Result<()> {
        require!(weight > 0, GovernanceError::InvalidVoteWeight);

        let proposal = &mut ctx.accounts.proposal;
        let voter_record = &mut ctx.accounts.voter_record;
        let clock = Clock::get()?;

        require!(
            proposal.status == ProposalStatus::Active,
            GovernanceError::ProposalNotActive
        );
        require!(
            clock.unix_timestamp < proposal.end_time,
            GovernanceError::VotingPeriodEnded
        );

        // Record vote
        voter_record.voter = ctx.accounts.voter.key();
        voter_record.proposal = proposal.key();
        voter_record.vote = vote;
        voter_record.weight = weight;

        // Update proposal vote counts
        match vote {
            Vote::For => proposal.votes_for += weight,
            Vote::Against => proposal.votes_against += weight,
        }

        msg!("Vote cast with weight {}", weight);
        Ok(())
    }

    pub fn finalize_proposal(ctx: Context<FinalizeProposal>) -> Result<()> {
        let dao = &ctx.accounts.dao;
        let proposal = &mut ctx.accounts.proposal;
        let clock = Clock::get()?;

        require!(
            proposal.status == ProposalStatus::Active,
            GovernanceError::ProposalNotActive
        );
        require!(
            clock.unix_timestamp >= proposal.end_time,
            GovernanceError::VotingPeriodNotEnded
        );

        let total_votes = proposal.votes_for + proposal.votes_against;
        
        // Check quorum (simplified - in production, use total supply)
        let quorum_met = total_votes > 0; // Placeholder logic

        if quorum_met {
            let approval_percentage = (proposal.votes_for as u128 * 100)
                .checked_div(total_votes as u128)
                .unwrap() as u8;

            if approval_percentage >= dao.approval_threshold {
                proposal.status = ProposalStatus::Passed;
                msg!("Proposal passed with {}% approval", approval_percentage);
            } else {
                proposal.status = ProposalStatus::Rejected;
                msg!("Proposal rejected - insufficient approval");
            }
        } else {
            proposal.status = ProposalStatus::Rejected;
            msg!("Proposal rejected - quorum not met");
        }

        Ok(())
    }

    pub fn execute_proposal(ctx: Context<ExecuteProposal>) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;

        require!(
            proposal.status == ProposalStatus::Passed,
            GovernanceError::ProposalNotPassed
        );
        require!(!proposal.executed, GovernanceError::AlreadyExecuted);

        // Execute proposal based on type
        match proposal.proposal_type {
            ProposalType::ParameterChange => {
                msg!("Executing parameter change proposal");
                // Implementation for parameter changes
            }
            ProposalType::Treasury => {
                msg!("Executing treasury proposal");
                // Implementation for treasury operations
            }
            ProposalType::Upgrade => {
                msg!("Executing upgrade proposal");
                // Implementation for contract upgrades
            }
        }

        proposal.executed = true;
        msg!("Proposal executed successfully");
        Ok(())
    }

    pub fn cancel_proposal(ctx: Context<CancelProposal>) -> Result<()> {
        let proposal = &mut ctx.accounts.proposal;

        require!(
            proposal.status == ProposalStatus::Active,
            GovernanceError::ProposalNotActive
        );

        proposal.status = ProposalStatus::Cancelled;

        msg!("Proposal cancelled");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitializeDao<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Dao::LEN,
        seeds = [b"dao"],
        bump
    )]
    pub dao: Account<'info, Dao>,

    #[account(mut)]
    pub authority: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CreateProposal<'info> {
    #[account(
        mut,
        seeds = [b"dao"],
        bump = dao.bump
    )]
    pub dao: Account<'info, Dao>,

    #[account(
        init,
        payer = proposer,
        space = 8 + Proposal::LEN,
        seeds = [b"proposal", dao.key().as_ref(), dao.proposal_count.to_le_bytes().as_ref()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(mut)]
    pub proposer: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub proposal: Account<'info, Proposal>,

    #[account(
        init,
        payer = voter,
        space = 8 + VoterRecord::LEN,
        seeds = [b"vote", proposal.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub voter_record: Account<'info, VoterRecord>,

    #[account(mut)]
    pub voter: Signer<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct FinalizeProposal<'info> {
    pub dao: Account<'info, Dao>,

    #[account(mut)]
    pub proposal: Account<'info, Proposal>,
}

#[derive(Accounts)]
pub struct ExecuteProposal<'info> {
    #[account(
        constraint = dao.authority == authority.key()
    )]
    pub dao: Account<'info, Dao>,

    #[account(mut)]
    pub proposal: Account<'info, Proposal>,

    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct CancelProposal<'info> {
    #[account(
        mut,
        constraint = proposal.proposer == proposer.key()
    )]
    pub proposal: Account<'info, Proposal>,

    pub proposer: Signer<'info>,
}

#[account]
pub struct Dao {
    pub authority: Pubkey,
    pub voting_period: i64,
    pub quorum_percentage: u8,
    pub approval_threshold: u8,
    pub proposal_count: u64,
    pub bump: u8,
}

impl Dao {
    pub const LEN: usize = 32 + 8 + 1 + 1 + 8 + 1;
}

#[account]
pub struct Proposal {
    pub dao: Pubkey,
    pub proposer: Pubkey,
    pub title: String,
    pub description: String,
    pub proposal_type: ProposalType,
    pub votes_for: u64,
    pub votes_against: u64,
    pub status: ProposalStatus,
    pub start_time: i64,
    pub end_time: i64,
    pub executed: bool,
    pub bump: u8,
}

impl Proposal {
    pub const LEN: usize = 32 + 32 + (4 + 100) + (4 + 500) + 1 + 8 + 8 + 1 + 8 + 8 + 1 + 1;
}

#[account]
pub struct VoterRecord {
    pub voter: Pubkey,
    pub proposal: Pubkey,
    pub vote: Vote,
    pub weight: u64,
}

impl VoterRecord {
    pub const LEN: usize = 32 + 32 + 1 + 8;
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ProposalType {
    ParameterChange,
    Treasury,
    Upgrade,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum ProposalStatus {
    Active,
    Passed,
    Rejected,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy)]
pub enum Vote {
    For,
    Against,
}

#[error_code]
pub enum GovernanceError {
    #[msg("Invalid voting period")]
    InvalidVotingPeriod,
    #[msg("Invalid quorum percentage")]
    InvalidQuorum,
    #[msg("Invalid approval threshold")]
    InvalidThreshold,
    #[msg("Title too long")]
    TitleTooLong,
    #[msg("Description too long")]
    DescriptionTooLong,
    #[msg("Proposal not active")]
    ProposalNotActive,
    #[msg("Voting period has ended")]
    VotingPeriodEnded,
    #[msg("Voting period has not ended")]
    VotingPeriodNotEnded,
    #[msg("Invalid vote weight")]
    InvalidVoteWeight,
    #[msg("Proposal has not passed")]
    ProposalNotPassed,
    #[msg("Proposal already executed")]
    AlreadyExecuted,
}
