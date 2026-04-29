// Data types for the insurance contract (Issue #101 - extracted from lib.rs)

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum PolicyStatus {
    Active,
    Expired,
    Cancelled,
    Claimed,
    Suspended,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum CoverageType {
    Fire,
    Flood,
    Earthquake,
    Theft,
    LiabilityDamage,
    NaturalDisaster,
    Comprehensive,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ClaimStatus {
    Pending,
    UnderReview,
    OracleVerifying,
    Approved,
    Rejected,
    Paid,
    Disputed,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum RiskLevel {
    VeryLow,
    Low,
    Medium,
    High,
    VeryHigh,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct InsurancePolicy {
    pub policy_id: u64,
    pub property_id: u64,
    pub policyholder: AccountId,
    pub coverage_type: CoverageType,
    pub coverage_amount: u128,
    pub premium_amount: u128,
    pub deductible: u128,
    pub start_time: u64,
    pub end_time: u64,
    pub status: PolicyStatus,
    pub risk_level: RiskLevel,
    pub pool_id: u64,
    pub claims_count: u32,
    pub total_claimed: u128,
    pub metadata_url: String,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct InsuranceClaim {
    pub claim_id: u64,
    pub policy_id: u64,
    pub claimant: AccountId,
    pub claim_amount: u128,
    pub description: String,
    pub evidence_url: String,
    pub oracle_report_url: String,
    pub status: ClaimStatus,
    pub submitted_at: u64,
    pub processed_at: Option<u64>,
    pub payout_amount: u128,
    pub assessor: Option<AccountId>,
    pub rejection_reason: String,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct RiskPool {
    pub pool_id: u64,
    pub name: String,
    pub coverage_type: CoverageType,
    pub total_capital: u128,
    pub available_capital: u128,
    pub total_premiums_collected: u128,
    pub total_claims_paid: u128,
    pub active_policies: u64,
    pub max_coverage_ratio: u32,
    pub reinsurance_threshold: u128,
    pub created_at: u64,
    pub is_active: bool,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct RiskAssessment {
    pub property_id: u64,
    pub location_risk_score: u32,
    pub construction_risk_score: u32,
    pub age_risk_score: u32,
    pub claims_history_score: u32,
    pub overall_risk_score: u32,
    pub risk_level: RiskLevel,
    pub assessed_at: u64,
    pub valid_until: u64,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct PremiumCalculation {
    pub base_rate: u32,
    pub risk_multiplier: u32,
    pub coverage_multiplier: u32,
    pub annual_premium: u128,
    pub monthly_premium: u128,
    pub deductible: u128,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ReinsuranceAgreement {
    pub agreement_id: u64,
    pub reinsurer: AccountId,
    pub coverage_limit: u128,
    pub retention_limit: u128,
    /// Basis points of premium to cede (e.g. 2000 = 20%). Used for QuotaShare.
    pub premium_ceded_rate: u32,
    pub coverage_types: Vec<CoverageType>,
    pub start_time: u64,
    pub end_time: u64,
    pub is_active: bool,
    pub total_ceded_premiums: u128,
    pub total_recoveries: u128,
    /// How risk is distributed with this reinsurer
    pub treaty_type: ReinsuranceTreatyType,
    /// Running count of premium cessions under this agreement
    pub cession_count: u64,
    /// Running count of loss recoveries under this agreement
    pub recovery_count: u64,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct InsuranceToken {
    pub token_id: u64,
    pub policy_id: u64,
    pub owner: AccountId,
    pub face_value: u128,
    pub is_tradeable: bool,
    pub created_at: u64,
    pub listed_price: Option<u128>,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ActuarialModel {
    pub model_id: u64,
    pub coverage_type: CoverageType,
    pub loss_frequency: u32,
    pub average_loss_severity: u128,
    pub expected_loss_ratio: u32,
    pub confidence_level: u32,
    pub last_updated: u64,
    pub data_points: u32,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct UnderwritingCriteria {
    pub max_property_age_years: u32,
    pub min_property_value: u128,
    pub max_property_value: u128,
    pub excluded_locations: Vec<String>,
    pub required_safety_features: bool,
    pub max_previous_claims: u32,
    pub min_risk_score: u32,
}

#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct PoolLiquidityProvider {
    pub provider: AccountId,
    pub pool_id: u64,
    pub deposited_amount: u128,
    pub share_percentage: u32,
    pub deposited_at: u64,
    pub last_reward_claim: u64,
    pub accumulated_rewards: u128,
}

// =========================================================================
// REINSURANCE DISTRIBUTION TYPES
// =========================================================================

/// Treaty type determines how risk is shared with the reinsurer
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
    ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub enum ReinsuranceTreatyType {
    /// Quota Share: cede a fixed % of every premium and claim
    QuotaShare,
    /// Excess of Loss: reinsurer covers losses above a retention threshold
    ExcessOfLoss,
    /// Surplus: cede the portion of risk exceeding the insurer's line
    Surplus,
}

/// Tracks a single premium cession event for audit purposes
#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct PremiumCession {
    pub cession_id: u64,
    pub agreement_id: u64,
    pub policy_id: u64,
    pub gross_premium: u128,
    pub ceded_premium: u128,
    pub ceded_at: u64,
}

/// Tracks a single loss recovery request from a reinsurer
#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct LossRecovery {
    pub recovery_id: u64,
    pub agreement_id: u64,
    pub claim_id: u64,
    pub gross_loss: u128,
    pub recovered_amount: u128,
    pub recovered_at: u64,
}

/// Summary statistics for a reinsurance agreement
#[derive(
    Debug, Clone, PartialEq, scale::Encode, scale::Decode, ink::storage::traits::StorageLayout,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct ReinsuranceStats {
    pub agreement_id: u64,
    pub treaty_type: ReinsuranceTreatyType,
    pub total_ceded_premiums: u128,
    pub total_recoveries: u128,
    pub cession_count: u64,
    pub recovery_count: u64,
    /// Net position: recoveries - ceded_premiums (can be negative conceptually, stored as i128)
    pub net_recovery: i128,
}
