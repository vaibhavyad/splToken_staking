use anchor_lang::prelude::*;
use anchor_spl::token;
use anchor_spl::token::Token;
use anchor_spl::token::TokenAccount;
// use anchor_spl::token::{ Token, TokenAccount};

declare_id!("BxnKuUL7pbhBgKbTaamn7qPgPtwaxHx6hJgDNoxhHjP2");


    // Helper function to get the daily reward rate based on stake in USDT
    pub fn get_daily_reward_rate(staked_usdt: u64) -> f64 {
        if staked_usdt >= 20 && staked_usdt < 2000 {
        0.002 // 0.2% daily for $20 - $2k
    } else if staked_usdt >= 2000 && staked_usdt < 10000 {
        0.0025 // 0.25% daily for $2k - $10k
    } else if staked_usdt >= 10000 && staked_usdt < 50000 {
        0.003 // 0.3% daily for $10k - $50k
    } else if staked_usdt >= 50000 && staked_usdt < 100000 {
        0.0035 // 0.35% daily for $50k - $100k
    } else if staked_usdt >= 100000{
        0.004 // 0.4% daily for > $100k
    }else {
        0.0
    }

    }

    fn calculate_elapsed_days(time: i64) -> f64 {
        let current_time = Clock::get().unwrap().unix_timestamp;
        let elapsed_seconds = current_time - time;
        elapsed_seconds as f64 / 86400.0
    } 

    // fn calculate_usdt_value(token_amount: u64) -> f64 {
    // let token_amount_in_standard = token_amount as f64 / 10u64.pow(9) as f64;
    // token_amount_in_standard * 15.0
    // }

#[program]
mod spl_token_staking {
    use super::*;

    pub fn initialize_pool(ctx: Context<InitializePool> ,staking_duration: i64) -> Result<()> {
        let staking_pool = &mut ctx.accounts.staking_pool;
        staking_pool.total_stakes_count = 0;
        staking_pool.total_staked = 0;
        staking_pool.staking_duration = staking_duration; //500 * 24 * 60 * 60 = 43200000 sec; // 500 days in seconds
        staking_pool.mint = ctx.accounts.pool_token_account.mint; // Store mint of staking token
        staking_pool.pool_authority = *ctx.accounts.pool_authority.key;

        Ok(())
    }

    pub fn initialize_staking_account(ctx: Context<InitializeStakingAccount>,owner: Pubkey) -> Result<()> {

        let staking_account = &mut ctx.accounts.staking_account;
        staking_account.stakes = Vec::new(); // Initialize as an empty vector
        staking_account.owner = owner;

        Ok(())

    }

    pub fn update_staking_duration(ctx: Context<UpdateStakingPool>, days: i64) -> Result<()> {
        
        let staking_pool = &mut ctx.accounts.staking_pool;

        require!(*ctx.accounts.pool_authority.key == staking_pool.pool_authority, ErrorCode::Unauthorized);

        staking_pool.staking_duration = days * 86400; // Convert minutes to seconds

        Ok(())
    }

    pub fn update_staking_period(ctx: Context<UpdateStakingPeriod>,stake_index: u32, new_staking_period: u64) -> Result<()> {
        let staking_account = &mut ctx.accounts.staking_account;
        let current_time = Clock::get()?.unix_timestamp;
        
        // Ensure the caller is the owner
        require!(
            staking_account.owner == *ctx.accounts.user.key,
            ErrorCode::Unauthorized
        );

        let stake_index_usize = stake_index as usize;
        let stake = staking_account
        .stakes
        .get_mut(stake_index_usize)
        .ok_or(ErrorCode::StakeNotFound)?;

        // Check if the user is trying to extend from 350 days to 500 days
        require!(
            stake.staking_period == 350 * 86400 && new_staking_period == 500,
            ErrorCode::InvalidStakingPeriodChange
        );

        // Verify that the current staking period has not completed
        require!(
            current_time - stake.staking_start_time < stake.staking_period,
            ErrorCode::StakingPeriodCompleted
        );
        
        // Update the staking period to the new period
        stake.staking_period = new_staking_period as i64 * 86400;
        
        Ok(())
    }

    // Stake tokens into the pool
    pub fn stake_tokens(ctx: Context<StakeTokens>, amount: u64, amount_in_usdt: u64,period: i64)  -> Result<()>{

        let clock = Clock::get()?;
        let staking_account = &mut ctx.accounts.staking_account;

        if  amount == 0 {
            return Err(ErrorCode::InvaildAmount.into());
        }

        let reward_rate = get_daily_reward_rate(amount_in_usdt); // Get the daily reward rate
        // Calculate reward based on staked amount and rate
        // let reward = (amount as f64 * reward_rate) as u64;

        let new_stake = StakeInfo {
        staked_amount:amount,
        staking_start_time: clock.unix_timestamp,
        staking_period:period * 86400,
        accumulated_rewards: reward_rate,
        last_claim_reward_time:0,
        last_claim_time:0,
        claim_reward: 0,
        claim_count:0,
         };
        staking_account.stakes.push(new_stake); // Add the new stake

        let cpi_accounts = token::Transfer {
                from: ctx.accounts.user_token_account.to_account_info(),
                to: ctx.accounts.pool_token_account.to_account_info(),
                authority: ctx.accounts.user.to_account_info(),
            };

        let transfer_ctx = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );

        // let transfer_ctx = ctx.accounts.into_transfer_to_pool_context(); // Borrow immutably here
        token::transfer(transfer_ctx, amount)?; // Transfer tokens to the pool
    
       // Update pool stats
        let staking_pool = &mut ctx.accounts.staking_pool;
        staking_pool.total_staked += amount;
        staking_pool.total_stakes_count += 1;

        Ok(())

    }

    // Calculate the daily reward in USDT based on the staked amount
    pub fn calculate_reward(ctx: Context<CalculateReward>, stake_index: u32, price: f64) -> Result<()> {
        let staking_account = &mut ctx.accounts.staking_account;

        let stake_index_usize = stake_index as usize;

        let stake_info = staking_account
        .stakes
        .get_mut(stake_index_usize)
        .ok_or(ErrorCode::StakeNotFound)?;

        // let rate = get_daily_reward_rate(stake_info.staked_amount);
        let elapsed_days = calculate_elapsed_days(stake_info.staking_start_time);
         msg!(
            "Emitting staking_start_time: {}",stake_info.staking_start_time,
        );
         msg!(
            "Emitting elapsed_days: {}",elapsed_days,
        );

        let reward = (stake_info.staked_amount as f64 * stake_info.accumulated_rewards * elapsed_days) as u64;
        let reward_in_usdt = price * reward as f64;

        msg!(
            "Emitting reward: {}",reward,
        );
        msg!(
            "Emitting reward in usdt : {}",reward_in_usdt,
        );
        Ok(())

    }

    pub fn claim_rewards(ctx: Context<ClaimRewards>, stake_index: u32,  price: f64) -> Result<()> {
        let staking_account = &mut ctx.accounts.staking_account;
        let stake_info = staking_account.stakes.get_mut(stake_index as usize).ok_or(ErrorCode::StakeNotFound)?;

        let current_time = Clock::get().unwrap().unix_timestamp;
         msg!(
            "Emitting xcheck : {}",current_time >= stake_info.staking_start_time+stake_info.staking_period,
        );
        require!(current_time <= stake_info.staking_start_time+stake_info.staking_period, ErrorCode::StakingPeriodCompleted);

        require!(current_time - stake_info.last_claim_reward_time >= 86400, ErrorCode::ClaimTooSoon);
      
        let elapsed_days = if stake_info.last_claim_reward_time == 0{
        calculate_elapsed_days(stake_info.staking_start_time)
        }else{
         calculate_elapsed_days(stake_info.last_claim_reward_time)
        };

        let reward = (stake_info.staked_amount as f64 * stake_info.accumulated_rewards * elapsed_days) as u64;
        let reward_in_usdt = reward as f64 * price;
        
        require!(reward_in_usdt >= 5.0, ErrorCode::MinimumRewardNotMet);

        let cpi_accounts = token::Transfer {
            from: ctx.accounts.reward_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.fee_payer.to_account_info(),
        };

        let transfer_ctx = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );

        token::transfer(transfer_ctx, reward)?; // Transfer reward tokens the user        
        
        stake_info.last_claim_reward_time = current_time;
        stake_info.claim_reward += reward;

        Ok(())
    }

    pub fn claim_rewards_after(ctx: Context<ClaimRewards>, stake_index: u32, price : f64) -> Result<()> {
        let staking_account = &mut ctx.accounts.staking_account;
        let stake_info = staking_account.stakes.get_mut(stake_index as usize).ok_or(ErrorCode::StakeNotFound)?;
        let current_time = Clock::get().unwrap().unix_timestamp;

        let staking_end_time = stake_info.staking_start_time+stake_info.staking_period;
        
        require!(current_time >= staking_end_time, ErrorCode::StakingPeriodNotCompleted);
        let is_staking_period_ended = current_time >= staking_end_time;

        msg!(
            "Emitting xcheck : {}",current_time >= staking_end_time
        );

         msg!(
            "Emitting is_staking_period_ended: {}",is_staking_period_ended,
        );

        let elapsed_seconds = if is_staking_period_ended {
            staking_end_time - stake_info.last_claim_reward_time
        } else {
            current_time - stake_info.last_claim_reward_time
        };
      msg!(
            "Emitting elapsed_seconds: {}",elapsed_seconds,
        );
        let elapsed_days = elapsed_seconds / 86400; // 86400 seconds in a day

         msg!(
            "Emitting elapsed_days: {}",elapsed_days,
        );

        require!(elapsed_days > 0, ErrorCode::ClaimTooSoon);
        // Calculate the total reward for the elapsed days
        let daily_reward = stake_info.staked_amount as f64 * stake_info.accumulated_rewards;
        let total_reward = (daily_reward * elapsed_days as f64 )as u64;

         msg!(
            "Emitting elapsed_days: {}",daily_reward,
        );

         msg!(
            "Emitting reward: {}",total_reward,
        );

        let token_amount_to_usd = total_reward as f64 * price;
        require!(is_staking_period_ended || token_amount_to_usd >= 5.0, ErrorCode::MinimumRewardNotMet);
        msg!(
            "Emitting token_amount_to_usd: {}",token_amount_to_usd,
        );

        let cpi_accounts = token::Transfer {
            from: ctx.accounts.reward_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.fee_payer.to_account_info(),
        };

        let transfer_ctx = CpiContext::new(
    ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );

        token::transfer(transfer_ctx, total_reward)?; // Transfer reward tokens the user      

        if is_staking_period_ended {
            stake_info.last_claim_reward_time = staking_end_time;
        } else {
            stake_info.last_claim_reward_time = current_time;
        }
        stake_info.claim_reward += total_reward;

        Ok(())
        
    }

    pub fn claim_staked_tokens(ctx: Context<ClaimStakedTokens>, stake_index: u32) -> Result<()> {
        let clock = Clock::get()?;
        let staking_account = &mut ctx.accounts.staking_account;
        let stake_index_usize = stake_index as usize;
        let stake = staking_account.stakes.get_mut(stake_index_usize)
            .ok_or(ErrorCode::StakeNotFound)?;

        let current_time = clock.unix_timestamp;
        let staking_end_time = stake.staking_start_time + stake.staking_period;
        let is_staking_period_ended = current_time >= staking_end_time;

        require!(is_staking_period_ended, ErrorCode::ClaimTooSoon);

        require!(stake.claim_count < 4, ErrorCode::AlreadyClaimedAll);
        
        let days_since_last_claim = (current_time - stake.last_claim_time) / 86400;
        require!(days_since_last_claim >= 1, ErrorCode::TooEarlyToClaim);
        
        let claimable_amount = stake.staked_amount / 4;

        let cpi_accounts = token::Transfer {
            from: ctx.accounts.pool_token_account.to_account_info(),
            to: ctx.accounts.user_token_account.to_account_info(),
            authority: ctx.accounts.staking_pool.to_account_info(),
        };
        // let seeds = &[b"staking_pool".as_ref(), &[ctx.accounts.staking_pool.bump]];
        // let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
        );
        token::transfer(cpi_ctx, claimable_amount)?;

        stake.claim_count += 1;
        stake.last_claim_time = current_time;
        Ok(())
    }

    #[derive(Accounts)]
    pub struct InitializePool<'info>{
        #[account(init, payer = user, space = 8 + std::mem::size_of::<StakingPool>())] // Storing total_staked and staking_duration
        pub staking_pool: Account<'info, StakingPool>,
        #[account(mut)]
        pub pool_token_account: Account<'info, TokenAccount>,
        /// CHECK: The `pool_authority` is a trusted address set during initialization and does not need further validation
        pub pool_authority: AccountInfo<'info>, // Add authority here
        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }
    
    #[derive(Accounts)]
    pub struct UpdateStakingPool<'info> {
        #[account(mut)]
        pub staking_pool: Account<'info, StakingPool>, // Mutable to allow changes
        pub pool_authority: Signer<'info>,              // Signer for authorization
    }

    #[derive(Accounts)]
    pub struct UpdateStakingPeriod<'info> {
    #[account(mut, has_one = owner)]
    pub staking_account: Account<'info, StakingAccount>,
    /// CHECK: This is the authority (user) making the request, verified in the handler
    pub user: AccountInfo<'info>,
    /// CHECK: This is the owner's public key, and its validity is assumed. Ensure that this is set and verified externally.
    pub owner: AccountInfo<'info>,
    #[account(signer)]
    pub fee_payer: Signer<'info>,
}
    
    #[derive(Accounts)]
    pub struct InitializeStakingAccount<'info> {
        #[account(
            init, 
            seeds = [b"staking", user.key().as_ref()],
            bump,
            payer = user, 
            space = 8 + std::mem::size_of::<StakingAccount>()+ 4 + (15 * std::mem::size_of::<StakeInfo>()), // Space for staked_amount and staking_start_time
        )]
        pub staking_account: Account<'info, StakingAccount>,
        #[account(mut)]
        pub user: Signer<'info>,
        pub system_program: Program<'info, System>,
    }

    #[derive(Accounts)]
    pub struct StakeTokens<'info>{
        #[account(mut)]
        pub staking_account: Account<'info, StakingAccount>,
        #[account(mut)]
        pub staking_pool: Account<'info, StakingPool>,
    #[account(
        mut,
        constraint = user_token_account.mint == staking_pool.mint
        )]    
        pub user_token_account: Account<'info, TokenAccount>, // User's token account
        #[account(mut)]
        pub pool_token_account: Account<'info, TokenAccount>, // Pool token account
        #[account(mut, signer)]
        pub user: Signer<'info>,
        // Fee-payer's wallet (hot wallet)
        // #[account(signer)]
        #[account(mut)]
        pub fee_payer: Signer<'info>,
        pub token_program: Program<'info, Token>,
    }

    #[derive(Accounts)]
    pub struct ClaimRewards<'info> {
        #[account(mut)]
        pub staking_account: Account<'info, StakingAccount>,
        #[account(mut)]
        pub reward_token_account: Account<'info, TokenAccount>,  // Reward token account
        #[account(mut)]
        pub user_token_account: Account<'info, TokenAccount>, // User's reward token account
        #[account(mut)]
        pub fee_payer: Signer<'info>,
        pub token_program: Program<'info, Token>,
    }

        
    #[derive(Accounts)]
    pub struct ClaimStakedTokens<'info> {
    #[account(mut)]
        pub staking_account: Account<'info, StakingAccount>,
        #[account(mut,signer)]
        pub staking_pool: Account<'info, StakingPool>,
        #[account(
        mut,
        constraint = user_token_account.mint == staking_pool.mint
        )]
        pub user_token_account: Account<'info, TokenAccount>, // User's token account
        #[account(mut)]
        pub pool_token_account: Account<'info, TokenAccount>, // Pool token account
        #[account(signer)]
        pub fee_payer: Signer<'info>,
        pub token_program: Program<'info, Token>,
    }

    #[account]
    pub struct StakingPool {
        pub total_stakes_count: u32,
        pub total_staked: u64,
        pub staking_duration: i64, // This could be a fixed size array for optimization
        pub mint: Pubkey,
        pub pool_authority:Pubkey,
    }
    
    #[account]
    pub struct StakingAccount {
        pub stakes: Vec<StakeInfo>, // A vector to track each individual stake
         /// CHECK: This is the owner's public key, and its validity is assumed. Ensure that this is set and verified externally.
        pub owner: Pubkey,
    }
    
    #[derive(AnchorSerialize, AnchorDeserialize, Clone)]
    pub struct StakeInfo {
        pub staked_amount: u64,
        pub staking_start_time: i64,
        pub staking_period: i64,
        pub last_claim_time:i64,
        pub last_claim_reward_time:i64,
        pub accumulated_rewards: f64,
        pub claim_reward:u64,
        pub claim_count:u8
    }
    #[derive(Accounts)]
    pub struct CalculateReward<'info> {
        #[account(mut)]
        pub staking_account: Account<'info, StakingAccount>,
    }


    #[error_code]
    pub enum ErrorCode {
        #[msg("The amount must be greater than zero.")]
        InvaildAmount,
        #[msg("Reward claim is too soon.")]
        ClaimTooSoon,
        #[msg("It is too early to claim tokens.")]
        TooEarlyToClaim,
        #[msg("You have already claimed all tokens.")]
        AlreadyClaimedAll,
        #[msg("It is not yet time to claim tokens.")]
        NotTimeToClaim,
        #[msg("Unauthorized")]
        Unauthorized,
        #[msg("Invalid staking period change.")]
        InvalidStakingPeriodChange,
        #[msg("Invalid staking period.")]
        InvalidStakingPeriod,
        #[msg("No staking found.")]
        StakeNotFound,
        #[msg("Staking Peroid is compleded.")]
        StakingPeriodCompleted,
        #[msg("Staking Peroid is not compleded.")]
        StakingPeriodNotCompleted,
        #[msg("The minimum claimable reward amount is not met.")]
        MinimumRewardNotMet,

    }

}  