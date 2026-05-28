use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid owner")]
    InvalidOwner,
    #[msg("Invalid update authority")]
    InvalidUpdateAuthority,
    #[msg("Asset is already staked")]
    AssetStaked,
    #[msg("Asset is not staked")]
    AssetNotStaked,
    #[msg("Freeze period has not passed")]
    FreezePeriodNotOver,
    #[msg("Invalid freeze period")]
    InvalidFreezePeriod,
    #[msg("Invalid rewards bps")]
    InvalidRewardsBps,
    #[msg("Invalid timestamp")]
    InvalidTimeStamp,
}
