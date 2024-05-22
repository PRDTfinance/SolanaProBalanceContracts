// #![warn(missing_docs)] // cannot be used because of how macros of anchor are used

//! # Contract Summary:
//!
//! This is a balance depositing contract for users. Users can deposit SOL or USDT into the contract.
//! Then they add withdraw requests on a backend and only the operator sends these balances to the users.
//! Admin wallet can withdraw any amount of SOL or USDT to his wallet.
//!
//! Deposit events emit an event so the backend can sync these and create balances on a centralized database accordingly.
//!
//! users can not call withdraw or sendWithdraw functions. Their requests are handled off chain and handled by master.operator wallet
//!
//! On contract creation, the deployer runs init_master to create master PDA. This PDA holds admin and operator wallets
//! The deployer then runs init_ata to create USDT ATA for master PDA.
//!
//! Master PDA keeps the SOL balance. Master PDA ATA keeps the USDT balance.
//!

use anchor_lang::{
    prelude::*,
    solana_program::{clock::Clock, program::invoke, pubkey::Pubkey, system_instruction::transfer},
};
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::*;
use std::mem::size_of;

declare_id!("8ZwcssGn5vKE1d6oBNNTTjDsFyTDKSuPtoooZQe9MHXb");

/// Master seed for the smart contract
pub const MASTER_SEED: &str = "master";

#[program]
mod pro_balance {
    use super::*;

    /// Will be run once after the deployment to set master PDA and setting admin operator wallets
    pub fn init_master(ctx: Context<InitMaster>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let operator = &ctx.accounts.operator;
        let admin = &ctx.accounts.admin;

        master.operator = operator.key();
        master.admin = admin.key();

        Ok(())
    }

    /// Will be run once to set USDT ATA
    pub fn init_ata(ctx: Context<InitAta>) -> Result<()> {
        let master = &mut ctx.accounts.master;

        if master.token_account.is_some() {
            return Err(Errors::TokenAccountAlreadyCreated.into());
        }

        master.token_account = Some(ctx.accounts.master_ata.key());

        Ok(())
    }

    /// this function is run by users to deposit SOL into the contract (master PDA balance)
    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let user = &ctx.accounts.user;

        invoke(
            &transfer(&user.key(), &master.key(), amount),
            &[
                user.to_account_info(),
                master.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        master.balance = master
            .balance
            .checked_add(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        let clock = Clock::get()?;

        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            holder: master.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    /// This function is run by users to deposit USDT into the contract (master PDA ATA balance)
    pub fn deposit_token(ctx: Context<DepositToken>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let from = &ctx.accounts.from;
        let to = &ctx.accounts.master_ata;
        let user = &ctx.accounts.user;

        let transfer_instruction = Transfer {
            from: from.to_account_info(),
            to: to.to_account_info(),
            authority: user.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, transfer_instruction);

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        master.token_balance = master
            .token_balance
            .checked_add(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        let clock = Clock::get()?;

        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            holder: to.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    /// This function can be called by master.admin to set a new operator
    pub fn set_operator(ctx: Context<SetOperator>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let operator = &ctx.accounts.new_operator;

        master.operator = operator.key();
        Ok(())
    }

    /// This function can be called by master.admin to transfer admin rights to a new wallet
    pub fn set_admin(ctx: Context<SetAdmin>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let admin = &ctx.accounts.new_admin;

        master.admin = admin.key();
        Ok(())
    }

    /// This function can be called by master.admin to withdraw any SOL amount to his wallet
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let admin = &mut ctx.accounts.admin;

        let rent_exemption = Rent::get()?.minimum_balance(MASTER_SIZE);
        require!(
            master.balance
                > amount
                    .checked_add(rent_exemption)
                    .map(Ok)
                    .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?,
            Errors::NotEnoughBalance
        );

        invoke(
            &transfer(&master.key(), &admin.key(), amount),
            &[
                master.to_account_info(),
                admin.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        master.balance = master
            .balance
            .checked_sub(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        let clock = Clock::get()?;

        emit!(AdminWithdrawEvent {
            user: admin.key(),
            holder: master.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    /// This function can be called by master.admin to withdraw any USDT amount to his wallet
    pub fn withdraw_token(ctx: Context<WithdrawToken>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let admin = &mut ctx.accounts.admin_ata;
        let from = &mut ctx.accounts.master_ata;

        let cpi_program = ctx.accounts.token_program.to_account_info();

        let seeds: &[&[&[u8]]] = &[&[MASTER_SEED.as_bytes(), &[ctx.bumps.master]]];

        let cpi_ctx = CpiContext::new_with_signer(
            cpi_program,
            Transfer {
                from: from.to_account_info(),
                to: admin.to_account_info(),
                authority: master.to_account_info(),
            },
            seeds,
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        master.token_balance = master
            .token_balance
            .checked_sub(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        let clock = Clock::get()?;

        emit!(AdminWithdrawEvent {
            user: admin.key(),
            holder: from.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    /// This function can be called by master.operator to send withdraw SOL amount to user wallet
    pub fn send_withdraw(ctx: Context<SendWithdraw>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let receiver = &mut ctx.accounts.receiver;

        let clock = Clock::get()?;
        master.last_withdraw_time = clock.unix_timestamp;

        let rent_exemption = Rent::get()?.minimum_balance(MASTER_SIZE);
        require!(
            master.balance
                > amount
                    .checked_add(rent_exemption)
                    .map(Ok)
                    .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?,
            Errors::NotEnoughBalance
        );

        invoke(
            &transfer(&master.key(), &receiver.key(), amount),
            &[
                master.to_account_info(),
                receiver.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        master.balance = master
            .balance
            .checked_sub(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        emit!(WithdrawEvent {
            user: receiver.key(),
            holder: master.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    /// This function can be called by master.operator to send withdraw USDT amount to user wallet
    pub fn send_withdraw_token(ctx: Context<SendWithdrawToken>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let receiver = &mut ctx.accounts.receiver_ata;
        let from = &mut ctx.accounts.master_ata;

        let clock = Clock::get()?;
        master.last_withdraw_time = clock.unix_timestamp;

        let cpi_program = ctx.accounts.token_program.to_account_info();

        let seeds: &[&[&[u8]]] = &[&[MASTER_SEED.as_bytes(), &[ctx.bumps.master]]];

        let cpi_ctx = CpiContext::new_with_signer(
            cpi_program,
            Transfer {
                from: from.to_account_info(),
                to: receiver.to_account_info(),
                authority: master.to_account_info(),
            },
            seeds,
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        master.token_balance = master
            .token_balance
            .checked_sub(amount)
            .map(Ok)
            .unwrap_or(Err(Errors::MathUnderflowOrOverflow))?;

        emit!(WithdrawEvent {
            user: receiver.key(),
            holder: from.key(),
            amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }
}

/// Errors of this smart contract.
#[error_code]
pub enum Errors {
    /// Unathorized access of the smart contract.
    #[msg("Unauthorized")]
    Unauthorized,
    /// Not enough balance
    #[msg("Not Enough Balance")]
    NotEnoughBalance,
    /// Token account has already been created.
    #[msg("Token account has already been created")]
    TokenAccountAlreadyCreated,
    /// Math underflow or overflow occurred
    #[msg("Math underflow or overflow occurred")]
    MathUnderflowOrOverflow,
}

/// Event of some deposit.
#[event]
pub struct DepositEvent {
    /// User which has deposited something.
    pub user: Pubkey,
    /// The account the deposit has been placed to.
    pub holder: Pubkey,
    /// Amount of SOL or token.
    pub amount: u64,
    /// When does the deposit event has happened.
    pub time: i64,
}

/// Event of a withdraw.
#[event]
pub struct WithdrawEvent {
    /// User which has withdrawn something.
    pub user: Pubkey,
    /// The account the withdraw has been taken tokens from.
    pub holder: Pubkey,
    /// Amount of SOL or token.
    pub amount: u64,
    /// When does the withdraw event has happened.
    pub time: i64,
}

/// Event of admin withdrawal.
#[event]
pub struct AdminWithdrawEvent {
    /// User which has withdrawn something.
    pub user: Pubkey,
    /// The account the withdraw has been taken tokens from.
    pub holder: Pubkey,
    /// Amount of SOL or token.
    pub amount: u64,
    /// When does the withdraw event has happened.
    pub time: i64,
}

const MASTER_SIZE: usize = size_of::<Master>() + 8;
/// `Master` account, which is the main account of the smart contract.
#[account]
pub struct Master {
    /// Solana stored in the smart contract.
    pub balance: u64,
    /// Tokens stored into the PDA of the smart contract.
    pub token_balance: u64,
    /// Associated token account for the master.
    pub token_account: Option<Pubkey>,
    /// Last time some withdraw has happen.
    pub last_withdraw_time: i64,
    /// Operator which is allowed to transfer token.
    pub operator: Pubkey,
    /// Admin which is allowed to manage the smart contract.
    pub admin: Pubkey,
}

/// Accounts for `InitMaster` instruction.
#[derive(Accounts)]
pub struct InitMaster<'info> {
    #[account(
        init,
        payer = payer,
        space = MASTER_SIZE,//8 + 8 + 32 + 32 + 8,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut)]
    pub payer: Signer<'info>,

    pub admin: SystemAccount<'info>,

    pub operator: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `InitAta` instruction.
#[derive(Accounts)]
pub struct InitAta<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(
        init,
        payer = user,
        associated_token::mint = token_mint,
        associated_token::authority = master,
        associated_token::token_program = token_program,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    #[account(mut, address=master.admin)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,

    pub associated_token_program: Program<'info, AssociatedToken>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `SetOperator` instruction.
#[derive(Accounts)]
pub struct SetOperator<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut, address=master.admin)]
    pub admin: Signer<'info>,

    pub new_operator: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `SetAdmin` instruction.
#[derive(Accounts)]
pub struct SetAdmin<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut, address=master.admin)]
    pub admin: Signer<'info>,

    pub new_admin: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `Deposit` instruction.
#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct Deposit<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `DepositToken` instruction.
#[derive(Accounts)]
pub struct DepositToken<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(
        mut,
        address=master.token_account.expect("token account has not been initialized"),
        associated_token::mint = token_mint,
        associated_token::authority = master,
        associated_token::token_program = token_program,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    #[account(mut)]
    pub from: Account<'info, TokenAccount>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

/// Accounts for `SendWithdraw` instruction.
#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct SendWithdraw<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut, address=master.operator)]
    pub operator: Signer<'info>,

    pub receiver: SystemAccount<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for Withdraw instruction.
#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct Withdraw<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(mut, address=master.admin)]
    pub admin: Signer<'info>,

    pub system_program: Program<'info, System>,
}

/// Accounts for WithdrawToken instruction.
#[derive(Accounts)]
pub struct WithdrawToken<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(
        mut,
        address=master.token_account.expect("token account has not been initialized"),
        associated_token::mint = token_mint,
        associated_token::authority = master,
        associated_token::token_program = token_program,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    #[account(mut, address=master.admin)]
    pub admin: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = admin,
        associated_token::token_program = token_program,
    )]
    pub admin_ata: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}

/// Accounts for SendWithdrawToken instruction.
#[derive(Accounts)]
pub struct SendWithdrawToken<'info> {
    #[account(
        mut,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,

    #[account(
        mut,
        address=master.token_account.expect("token account has not been initialized"),
        associated_token::mint = token_mint,
        associated_token::authority = master,
        associated_token::token_program = token_program,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    #[account(mut, address=master.operator)]
    pub operator: Signer<'info>,

    #[account(
        mut,
        associated_token::mint = token_mint,
        associated_token::authority = receiver,
        associated_token::token_program = token_program,
    )]
    pub receiver_ata: Account<'info, TokenAccount>,

    pub receiver: SystemAccount<'info>,

    pub token_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,

    pub system_program: Program<'info, System>,
}
