/*
Contract Summary:

This is a balance depositing contract for users. Users can deposit SOL or USDT into the contract. 
Then they add withdraw requests on a backend and only the operator sends these balances to the users.
Admin wallet can withdraw any amount of SOL or USDT to his wallet.

Deposit events emit an event so the backend can sync these and create balances on a centralized database accordingly.

users can not call withdraw or sendWithdraw functions. Their requests are handled off chain and handled by master.operator wallet

On contract creation, the deployer runs init_master to create master PDA. This PDA holds admin and operator wallets
The deployer then runs init_ata to create USDT ATA for master PDA.

Master PDA keeps the SOL balance. Master PDA ATA keeps the USDT balance.

--------

Anchor version: anchor-cli 0.29.0

--------

Cargo.toml:

[package]
name = "pro-balance"
version = "0.1.0"
description = "Created with Anchor"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]
name = "pro_balance"

[features]
no-entrypoint = []
no-idl = []
no-log-ix-name = []
cpi = ["no-entrypoint"]
default = []

[dependencies]
anchor-lang = "0.29.0"
anchor-spl = "0.29.0"
solana-program = "*"
spl-associated-token-account = "*"

*/

use anchor_lang::{
    prelude::*,
    solana_program::{
        clock::Clock, program::invoke, pubkey::Pubkey, system_instruction::transfer,
    },
};
use std::mem::size_of;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::*;

declare_id!("8ZwcssGn5vKE1d6oBNNTTjDsFyTDKSuPtoooZQe9MHXb");

pub const MASTER_SEED: &str = "master";

#[program]
mod pro_balance {
    use super::*;

    //will be run once after the deployment to set master PDA and setting admin operator wallets
    pub fn init_master(ctx: Context<InitMaster>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let operator = &ctx.accounts.operator;
        let admin = &ctx.accounts.admin;
        
        master.operator = operator.key();
        master.admin = admin.key();

        Ok(())
    }

    //will be run once to set USDT ATA
    pub fn init_ata(_ctx: Context<InitAta>) -> Result<()> {
        Ok(())
    }

    //this function is run by users to deposit SOL into the contract (master PDA balance)
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

        master.balance += amount;

        let clock = Clock::get()?;

        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            holder: master.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    //this function is run by users to deposit USDT into the contract (master PDA ATA balance)
    pub fn deposit_token(ctx: Context<DepositToken>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let from = &ctx.accounts.from;
        let to = &ctx.accounts.master_ata;
        let user = &ctx.accounts.user;

        let transfer_instruction = Transfer{
           from: from.to_account_info(),
           to: to.to_account_info(),
           authority: user.to_account_info(),
        };

        let cpi_program = ctx.accounts.token_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, transfer_instruction);

        anchor_spl::token::transfer(cpi_ctx, amount)?;

        master.token_balance += amount;

        let clock = Clock::get()?;
        
        emit!(DepositEvent {
            user: ctx.accounts.user.key(),
            holder: to.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    //this function can be called by master.admin to set a new operator
    pub fn set_operator(ctx: Context<SetOperator>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let operator = &ctx.accounts.new_operator;
        
        master.operator = operator.key();
        Ok(())
    }

    //this function can be called by master.admin to transfer admin rights to a new wallet
    pub fn set_admin(ctx: Context<SetAdmin>) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let admin = &ctx.accounts.new_admin;
        
        master.admin = admin.key();
        Ok(())
    }

    //this function can be called by master.admin to withdraw any SOL amount to his wallet
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let admin = &mut ctx.accounts.admin;

        let rent_exemption = Rent::get()?.minimum_balance(size_of::<Master>() + 8);
        require!(master.balance > amount + rent_exemption, Errors::NotEnoughBalance);

        master.sub_lamports(amount)?;
        admin.add_lamports(amount)?;

        master.balance -= amount;

        let clock = Clock::get()?;

        emit!(AdminWithdrawEvent {
            user: admin.key(),
            holder: master.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    //this function can be called by master.admin to withdraw any USDT amount to his wallet
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
            seeds
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;
 
        master.token_balance -= amount;


        let clock = Clock::get()?;

        emit!(AdminWithdrawEvent {
            user: admin.key(),
            holder: from.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    //this function can be called by master.operator to send withdraw SOL amount to user wallet
    pub fn send_withdraw(ctx: Context<SendWithdraw>, amount: u64) -> Result<()> {
        let master = &mut ctx.accounts.master;
        let receiver = &mut ctx.accounts.receiver;

        let clock = Clock::get()?;
        master.last_withdraw_time = clock.unix_timestamp;

        let rent_exemption = Rent::get()?.minimum_balance(size_of::<Master>() + 8);
        require!(master.balance > amount + rent_exemption, Errors::NotEnoughBalance);

        **master.to_account_info().try_borrow_mut_lamports()? -= amount;
        **receiver.to_account_info().try_borrow_mut_lamports()? += amount;

        master.balance -= amount;

        emit!(WithdrawEvent {
            user: receiver.key(),
            holder: master.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }

    //this function can be called by master.operator to send withdraw USDT amount to user wallet
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
            seeds
        );

        anchor_spl::token::transfer(cpi_ctx, amount)?;
 
        master.token_balance -= amount;

        emit!(WithdrawEvent {
            user: receiver.key(),
            holder: from.key(),
            amount: amount,
            time: clock.unix_timestamp,
        });

        Ok(())
    }
}

#[error_code]
pub enum Errors {
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Not Enough Balance")]
    NotEnoughBalance,
}

#[event]
pub struct DepositEvent {
    pub user: Pubkey,
    pub holder: Pubkey,
    pub amount: u64,
    pub time: i64,
}

#[event]
pub struct WithdrawEvent {
    pub user: Pubkey,
    pub holder: Pubkey,
    pub amount: u64,
    pub time: i64,
}

#[event]
pub struct AdminWithdrawEvent {
    pub user: Pubkey,
    pub holder: Pubkey,
    pub amount: u64,
    pub time: i64,
}

#[derive(Accounts)]
pub struct InitMaster<'info> {
    #[account(
        init,
        payer = payer,
        space = size_of::<Master>() + 8,//8 + 8 + 32 + 32 + 8,
        seeds = [MASTER_SEED.as_bytes()],
        bump,
    )]
    pub master: Account<'info, Master>,
    #[account(mut)]
    pub payer: Signer<'info>,


    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub admin: AccountInfo<'info>,

    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub operator: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

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
    )]
    pub master_ata: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,

    #[account(mut)]
    pub user: Signer<'info>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

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
    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub new_operator: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

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
    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub new_admin: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
pub struct Master {
    pub balance: u64,
    pub token_balance: u64,
    pub last_withdraw_time: i64,
    pub operator: Pubkey,
    pub admin: Pubkey,
}

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
        associated_token::mint = token_mint,
        associated_token::authority = master,
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
    #[account(mut)]
    /// CHECK: This is not dangerous because we don't read or write from this account
    pub receiver: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}

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
        associated_token::mint = token_mint,
        associated_token::authority = master,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    #[account(mut, address=master.admin)]
    pub admin: Signer<'info>,
    #[account(mut)]
    pub admin_ata: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

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
        associated_token::mint = token_mint,
        associated_token::authority = master,
    )]
    pub master_ata: Account<'info, TokenAccount>,

    #[account(mut, address=master.operator)]
    pub operator: Signer<'info>,

    #[account(mut)]
    pub receiver_ata: Account<'info, TokenAccount>,

    pub token_mint: Account<'info, Mint>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}