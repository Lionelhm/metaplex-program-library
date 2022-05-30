use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use mpl_token_metadata::instruction::thaw_delegated_account;
use solana_program::program::{invoke, invoke_signed};
use spl_token::instruction::revoke;

use crate::{cmp_pubkeys, constants::FREEZE, CandyError, CandyMachine, FreezePDA};

/// Set the collection PDA for the candy machine
#[derive(Accounts)]
pub struct ThawNFT<'info> {
    #[account(mut, seeds = [b"freeze".as_ref(), candy_machine.key.as_ref()], bump, has_one = candy_machine)]
    freeze_pda: Account<'info, FreezePDA>,
    /// CHECK: account could be empty so must be unchecked. Checked in freeze_pda constraint.
    #[account(mut)]
    candy_machine: UncheckedAccount<'info>,
    #[account(mut, has_one = mint, has_one = owner)]
    token_account: Account<'info, TokenAccount>,
    /// CHECK: checked in token_account constraints
    owner: UncheckedAccount<'info>,
    mint: Account<'info, Mint>,
    /// CHECK: account checked in CPI
    edition: UncheckedAccount<'info>,
    #[account(mut)]
    payer: Signer<'info>,
    token_program: Program<'info, Token>,
    /// CHECK: checked in account constraints
    #[account(address = mpl_token_metadata::id())]
    token_metadata_program: UncheckedAccount<'info>,
}

pub fn handle_thaw_nft(ctx: Context<ThawNFT>) -> Result<()> {
    let freeze_pda = &ctx.accounts.freeze_pda;
    let candy_machine = &mut ctx.accounts.candy_machine;
    let mut can_thaw = freeze_pda.allow_thaw;
    msg!("Can thaw: {}", can_thaw);
    if !can_thaw {
        if candy_machine.data_is_empty() {
            can_thaw = true;
        } else {
            let data = candy_machine.try_borrow_data()?;
            let candy_struct: CandyMachine = CandyMachine::try_deserialize(&mut data.as_ref())?;
            if candy_struct.items_redeemed == candy_struct.data.items_available {
                can_thaw = true;
            }
        }
    }
    if !can_thaw {
        return err!(CandyError::InvalidThawNFT);
    }
    let token_account = &ctx.accounts.token_account;
    let mint = &ctx.accounts.mint;
    let edition = &ctx.accounts.edition;
    let payer = &ctx.accounts.payer;
    let owner = &ctx.accounts.owner;
    let token_program = &ctx.accounts.token_program;
    let token_metadata_program = &ctx.accounts.token_metadata_program;
    let freeze_seeds = [
        FREEZE.as_bytes(),
        candy_machine.key.as_ref(),
        &[*ctx.bumps.get("freeze_pda").unwrap()],
    ];
    if token_account.is_frozen() {
        msg!("Token account is frozen! Now attempting to thaw!");
        invoke_signed(
            &thaw_delegated_account(
                mpl_token_metadata::ID,
                freeze_pda.key(),
                token_account.key(),
                edition.key(),
                mint.key(),
            ),
            &[
                freeze_pda.to_account_info(),
                token_account.to_account_info(),
                edition.to_account_info(),
                mint.to_account_info(),
                token_program.to_account_info(),
                token_metadata_program.to_account_info(),
            ],
            &[&freeze_seeds],
        )?;
    } else {
        msg!("Token account is not frozen!");
    }
    if cmp_pubkeys(&payer.key(), &owner.key()) {
        msg!("Revoking authority");
        invoke(
            &revoke(&spl_token::ID, &token_account.key(), &payer.key(), &[])?,
            &[token_account.to_account_info(), payer.to_account_info()],
        )?;
    } else {
        msg!("Cannot revoke delegate authority: token account owner is not signer. Rerun as owner to revoke");
    }
    Ok(())
}
