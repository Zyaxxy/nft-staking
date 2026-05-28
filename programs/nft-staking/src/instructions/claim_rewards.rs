use anchor_lang::prelude::*;
use anchor_spl::{associated_token::AssociatedToken, token_interface::{Mint, MintToChecked, TokenAccount, TokenInterface, mint_to_checked}};
use crate::{error::ErrorCode, constants::SECOND_PER_DAY};
use mpl_core::{
    ID as MPL_CORE_PROGRAM_ID, accounts::{BaseAssetV1, BaseCollectionV1}, fetch_plugin, instructions::{UpdatePluginV1CpiBuilder}, types::{Attribute, Attributes, Plugin, PluginType, UpdateAuthority}
};
use crate::Config;

#[derive(Accounts)]
pub struct ClaimRewards<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(
        seeds = [b"config", collection.key().as_ref()],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        has_one = owner @ ErrorCode::InvalidOwner,
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ ErrorCode::InvalidUpdateAuthority
    )]
    pub asset: Account<'info, BaseAssetV1>,
    #[account(
        mut,
        has_one = update_authority @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub collection: Account<'info, BaseCollectionV1>,
    ///CHECK: This account is not being read or written to, we only need to verify that the asset's update authority is correct
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"rewards_mint", config.key().as_ref()],
        bump = config.rewards_bump,
    )]
    pub rewards_mint: InterfaceAccount<'info, Mint>,
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = rewards_mint,
        associated_token::authority = owner,
    )]
    pub user_rewards_ata: InterfaceAccount<'info, TokenAccount>,
    pub token_program: Interface<'info, TokenInterface>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
    ///CHECK: THIS IS MPL CORE PROGRAM
    #[account(address = Pubkey::from(MPL_CORE_PROGRAM_ID.to_bytes()))]
    pub mpl_core_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<ClaimRewards>) -> Result<()> {
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_,attrs,_)| attrs);

    require!(attributes_fetched.is_some(), ErrorCode::AssetNotStaked);
    let attributes = attributes_fetched.unwrap();
    let mut attributes_list: Vec<Attribute> = Vec::with_capacity(attributes.attribute_list.len());

    let current_timestamp = Clock::get()?.unix_timestamp;
    let mut staked_timestamp: i64 = 0;
    let mut staked_time: i64 = 0;

    for attribute in &attributes.attribute_list {
        if attribute.key == "staked" {
            require!(attribute.value == "true", ErrorCode::AssetNotStaked);
            attributes_list.push(attribute.clone());
        } else if attribute.key == "staked_at" {
            staked_timestamp = staked_timestamp
                .checked_add(attribute.value.parse::<i64>().map_err(|_| ErrorCode::InvalidTimeStamp)?)
                .ok_or(ErrorCode::InvalidTimeStamp)?;
            // calculate the staked time in seconds
            staked_time = current_timestamp
                .checked_sub(staked_timestamp)
                .ok_or(ErrorCode::InvalidTimeStamp)?;
            // convert the staked time to days
            staked_time = staked_time
                .checked_div(SECOND_PER_DAY)
                .ok_or(ErrorCode::InvalidTimeStamp)?;
            require!(
                staked_time >= ctx.accounts.config.freeze_period as i64,
                ErrorCode::FreezePeriodNotOver
            );
        } else {
            attributes_list.push(attribute.clone());
        }
    }

    // Calculate rewards
    let amount = (staked_time as u64)
        .checked_mul(ctx.accounts.config.rewards_bps as u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_mul(10u64.pow(ctx.accounts.rewards_mint.decimals as u32))
        .ok_or(ErrorCode::InvalidRewardsBps)?
        .checked_div(10000u64)
        .ok_or(ErrorCode::InvalidRewardsBps)?;

    // Prepare signer seeds for config
    let collection_key = ctx.accounts.collection.key();
    let config_seeds = &[
        b"config".as_ref(),
        collection_key.as_ref(),
        &[ctx.accounts.config.bump],
    ];
    let config_signer = &[&config_seeds[..]];

    // Mint rewards to user
    mint_to_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            MintToChecked {
                mint: ctx.accounts.rewards_mint.to_account_info(),
                to: ctx.accounts.user_rewards_ata.to_account_info(),
                authority: ctx.accounts.config.to_account_info(),
            },
            config_signer,
        ),
        amount,
        ctx.accounts.rewards_mint.decimals,
    )?;

    // Update staked_at timestamp to current timestamp to reset the clock
    attributes_list.push(Attribute {
        key: "staked_at".to_string(),
        value: current_timestamp.to_string(),
    });

    // Prepare signer seeds for update_authority
    let signer_seeds = &[
        b"update_authority".as_ref(),
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    // Update the Asset with new staked_at timestamp
    UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.update_authority.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .plugin(Plugin::Attributes(Attributes {
            attribute_list: attributes_list,
        }))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .invoke_signed(&[signer_seeds])?;

    Ok(())
}
