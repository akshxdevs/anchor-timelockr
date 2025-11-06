#![allow(unexpected_cfgs,deprecated)]
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token, Transfer};

declare_id!("8tkSQhu3jiQqk2dbHPP2W4Jipdp3yg5a1JY6YQeRf6tB");

#[program]
pub mod anchor_timelockr {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>,unlock_time:i64, backup_adr:Pubkey) -> Result<()> {
        ctx.accounts.vault.owner = ctx.accounts.user.key();
        ctx.accounts.vault.unlock_time = unlock_time;
        ctx.accounts.vault.recovery_enabled = false;
        ctx.accounts.vault.bump = ctx.bumps.vault;
        ctx.accounts.vault.amount = 0;
        ctx.accounts.vault.backup_adr = backup_adr;

        Ok(())
    }
    pub fn deposite (ctx: Context<Deposite>,amount:u64) -> Result<()> {
        let cpi_accounts = Transfer {
            from: ctx.accounts.user_ata.to_account_info(),
            to: ctx.accounts.vault_ata.to_account_info(),
            authority: ctx.accounts.user.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.token_program.to_account_info(), cpi_accounts);
        token::transfer(cpi_ctx, amount)?;
        ctx.accounts.vault.amount += amount;
        Ok(())
    } 
    pub fn trigger_recovery(ctx: Context<TriggerRecovery>) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        let signer = &ctx.accounts.user;
    
        require_keys_eq!(
            vault.backup_adr,
            signer.key(),
            CustomError::NotAuthorized
        );
    
        let current_time = Clock::get()?.unix_timestamp;
        vault.recovery_enabled = true;
        vault.recovery_req_time = current_time
            .checked_add(10)
            .ok_or(CustomError::Overflow)?;
    
        Ok(())
    }
    
    pub fn withdrawl(ctx: Context<Withdrawl>)->Result<()> {
        let now = Clock::get()?.unix_timestamp;
        let amount = ctx.accounts.vault.amount;
        let unlock_time = ctx.accounts.vault.unlock_time;
        let rec_enabled = ctx.accounts.vault.recovery_enabled;
        let rec_time = ctx.accounts.vault.recovery_req_time;
        let vault_owner = ctx.accounts.vault.owner;
        let vault_bump = ctx.accounts.vault.bump;
        let reduced_fee = amount * 10/100;
        // Validate owner account
        require_keys_eq!(ctx.accounts.owner.key(), vault_owner, CustomError::NotAuthorized);
        
        // Check authorization and timing
        if ctx.accounts.user.key() == vault_owner {
            require!(now >= unlock_time, CustomError::UnlockTimeNotReached);
        } else if ctx.accounts.user.key() == ctx.accounts.vault.backup_adr {
            require!(rec_enabled, CustomError::RecoveryNotTriggered);
            require!(now >= rec_time, CustomError::RecoveryIsNotFinished);
        }
        
        // Transfer tokens using correct vault seeds
        let vault_seeds = &[b"vault", vault_owner.as_ref(), &[vault_bump]];
        let signer = &[&vault_seeds[..]];
        ctx.accounts.vault.amount = 0;  

        let cpi_accounts = Transfer {
            from: ctx.accounts.vault_ata.to_account_info(),
            to: ctx.accounts.user_ata.to_account_info(),
            authority: ctx.accounts.vault.to_account_info(),
        };
        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            cpi_accounts,
            signer,
        );
        token::transfer(cpi_ctx, amount-reduced_fee)?;   
        Ok(())
    }
}

#[account]
pub struct Vault{
    pub owner:Pubkey,
    pub amount:u64,
    pub backup_adr:Pubkey,
    pub unlock_time:i64,
    pub recovery_enabled:bool,
    pub recovery_req_time:i64,
    pub bump:u8,
}
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        seeds = [b"vault",user.key().as_ref()],
        payer = user,
        space = 8 + 90,
        bump
    )]
    pub vault:Account<'info,Vault>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program:Program<'info,System>,
}
#[derive(Accounts)]
pub struct Deposite<'info> {
    /// CHECK: vault_ata is a token account owned by the vault PDA
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,

    /// CHECK: user_ata is a token account owned by the user
    #[account(mut)]
    pub user_ata: AccountInfo<'info>,

    #[account(mut, seeds = [b"vault", user.key().as_ref()], bump)]
    pub vault: Account<'info, Vault>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct Withdrawl<'info>{
    /// CHECK: This is the owner of the vault, validated manually
    #[account(mut)]
    pub owner:AccountInfo<'info>,   
    /// CHECK: vault_ata is a token account owned by the vault PDA
    #[account(mut)]
    pub vault_ata: AccountInfo<'info>,
    /// CHECK: user_ata is a token account owned by the user
    #[account(mut)]
    pub user_ata: AccountInfo<'info>,
    #[account(mut)]
    pub vault:Account<'info,Vault>,
    pub user:Signer<'info>,
    pub system_program:Program<'info,System>,
    pub token_program:Program<'info,Token>
}

#[derive(Accounts)]
pub struct TriggerRecovery<'info> {
    #[account(mut)]
    pub vault: Account<'info, Vault>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
#[error_code]
pub enum CustomError {
    #[msg("User not authorzied to deposite or withdrawl funds!")]
    NotAuthorized,
    #[msg("recovery time is not yet finished!")]
    RecoveryIsNotFinished,
    #[msg("Somethig wrong happened during recovery!")]
    NotAbleToRecover,
    #[msg("Unlock time not yet reached.")]
    UnlockTimeNotReached,
    #[msg("Recovery process not triggered yet.")]
    RecoveryNotTriggered,
    #[msg("Overflow error")]
    Overflow,

}