use anchor_lang::prelude::*;
use mpl_core::{
    ID as MPL_CORE_PROGRAM_ID, accounts::{BaseAssetV1, BaseCollectionV1}, fetch_plugin, instructions::{AddPluginV1CpiBuilder, UpdatePluginV1CpiBuilder}, types::{Attribute, Attributes, FreezeDelegate, Plugin, PluginAuthority, PluginType, UpdateAuthority}
};

use crate::state::Config;
use crate::error::ErrorCode;

#[derive(Accounts)]
pub struct Stake<'info> {
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
        constraint = asset.update_authority == UpdateAuthority::Collection(collection.key()) @ ErrorCode::InvalidUpdateAuthority,
    )]
    pub asset: Account<'info, BaseAssetV1>,
    #[account(mut,
    has_one = update_authority @ ErrorCode::InvalidUpdateAuthority)]
    pub collection: Account<'info, BaseCollectionV1>,
    ///CHECK: This account is not initialized and it is used for signing purposes only, we verify that derives from the correct seeds
    #[account(
        seeds = [b"update_authority", collection.key().as_ref()],
        bump,
    )]
    pub update_authority: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    ///CHECK: This is the ID of the MPL core program
    #[account(address = MPL_CORE_PROGRAM_ID)]
    pub mpl_core_program: UncheckedAccount<'info>,
}

pub fn handler(ctx: Context<Stake>) -> Result<()> {
    
    let attributes_fetched: Option<Attributes> = fetch_plugin::<BaseAssetV1, Attributes>(
        &ctx.accounts.asset.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_,attrs,_)| attrs);
    //prepare the list of attributes to be added/updated
    let mut attributes_list: Vec<Attribute> = Vec::new();
    if let Some(attributes) = &attributes_fetched {
        for attribute in &attributes.attribute_list{
            if attribute.key == "staked"{
                require!(attribute.value == "true", ErrorCode::AssetStaked);
            }
            else if attribute.key == "staked_at" {
                attributes_list.push(attribute.clone());
            }
        }
    }

    attributes_list.push(Attribute { key: "staked".to_string(), value: "true".to_string() });
    attributes_list.push(Attribute { key: "staked_at".to_string(), value: Clock::get()?.unix_timestamp.to_string() });

    let collection_key = ctx.accounts.collection.key();
    let signer_seeds = &[
        b"update_authority".as_ref(),
        collection_key.as_ref(),
        &[ctx.bumps.update_authority],
    ];

    if attributes_fetched.is_none(){
        AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
            .asset(&ctx.accounts.asset.to_account_info())
            .collection(Some(&ctx.accounts.collection.to_account_info()))
            .authority(Some(&ctx.accounts.update_authority.to_account_info()))
            .payer(&ctx.accounts.owner.to_account_info())
            .plugin(Plugin::Attributes(Attributes { attribute_list: attributes_list }))
            .system_program(&ctx.accounts.system_program.to_account_info())
            .init_authority(PluginAuthority::UpdateAuthority)
            .invoke_signed(&[signer_seeds])?;
    }
    else {
        UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
            .asset(&ctx.accounts.asset.to_account_info())
            .collection(Some(&ctx.accounts.collection.to_account_info()))
            .authority(Some(&ctx.accounts.update_authority.to_account_info()))
            .payer(&ctx.accounts.owner.to_account_info())
            .plugin(Plugin::Attributes(Attributes { attribute_list: attributes_list }))
            .system_program(&ctx.accounts.system_program.to_account_info())
            .invoke_signed(&[signer_seeds])?;
    }

    AddPluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
        .asset(&ctx.accounts.asset.to_account_info())
        .collection(Some(&ctx.accounts.collection.to_account_info()))
        .authority(Some(&ctx.accounts.owner.to_account_info()))
        .payer(&ctx.accounts.owner.to_account_info())
        .plugin(Plugin::FreezeDelegate(FreezeDelegate { frozen: true }))
        .system_program(&ctx.accounts.system_program.to_account_info())
        .init_authority(PluginAuthority::UpdateAuthority)
        .invoke()?;

    // Update collection's Attributes plugin to increment staked NFTs count
    let collection_attributes_fetched: Option<Attributes> = fetch_plugin::<BaseCollectionV1, Attributes>(
        &ctx.accounts.collection.to_account_info(),
        PluginType::Attributes,
    )
    .ok()
    .map(|(_,attrs,_)| attrs);

    if let Some(collection_attributes) = collection_attributes_fetched {
        let mut collection_attributes_list: Vec<Attribute> = Vec::new();
        let mut staked_count: u64 = 0;

        for attribute in &collection_attributes.attribute_list {
            if attribute.key == "staked_nfts_count" {
                staked_count = attribute.value.parse::<u64>()
                    .map_err(|_| ErrorCode::InvalidTimeStamp)?;
            } else {
                collection_attributes_list.push(attribute.clone());
            }
        }

        staked_count = staked_count.checked_add(1)
            .ok_or(ErrorCode::InvalidTimeStamp)?;
        collection_attributes_list.push(Attribute {
            key: "staked_nfts_count".to_string(),
            value: staked_count.to_string(),
        });

        UpdatePluginV1CpiBuilder::new(&ctx.accounts.mpl_core_program.to_account_info())
            .collection(Some(&ctx.accounts.collection.to_account_info()))
            .authority(Some(&ctx.accounts.update_authority.to_account_info()))
            .payer(&ctx.accounts.owner.to_account_info())
            .plugin(Plugin::Attributes(Attributes {
                attribute_list: collection_attributes_list,
            }))
            .system_program(&ctx.accounts.system_program.to_account_info())
            .invoke_signed(&[signer_seeds])?;
    }

    Ok(())
}       