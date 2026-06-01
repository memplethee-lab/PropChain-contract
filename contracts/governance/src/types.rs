// Data types for the governance contract (Issue #101 - extracted from lib.rs)

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
pub enum GovernanceAction {
    ModifyProperty,
    SaleApproval,
    ChangeThreshold,
    AddSigner,
    RemoveSigner,
    EmergencyOverride,
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
pub enum ProposalStatus {
    Active,
    Approved,
    Executed,
    Rejected,
    Cancelled,
    Expired,
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
pub struct GovernanceProposal {
    pub id: u64,
    pub proposer: AccountId,
    pub description_hash: Hash,
    pub action_type: GovernanceAction,
    pub target: Option<AccountId>,
    pub threshold: u32,
    pub votes_for: u32,
    pub votes_against: u32,
    pub status: ProposalStatus,
    pub created_at: u64,
    pub executed_at: u64,
    pub timelock_until: u64,
    pub is_emergency: bool,
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    scale::Encode,
    scale::Decode,
)]
#[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
pub struct GovernanceAnalytics {
    pub total_proposals: u64,
    pub executed_proposals: u64,
    pub rejected_proposals: u64,
    pub cancelled_proposals: u64,
    pub active_proposals: u64,
    pub avg_participation_bps: u32,
}

// ── Proposal Template Types (Issue #230) ────────────────────────────────────

/// A reusable template for creating governance proposals.
/// Templates capture common proposal patterns (e.g. "Add signer",
/// "Change threshold") with a description template and default parameters.
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
pub struct ProposalTemplate {
    /// Unique template ID
    pub id: u64,
    /// Human-readable name (e.g. "Add a New Signer")
    pub name: String,
    /// Short description of what this template does
    pub description: String,
    /// Pre-configured action type
    pub action_type: GovernanceAction,
    /// Optional default target
    pub default_target: Option<AccountId>,
    /// Whether this template creates emergency proposals
    pub is_emergency: bool,
    /// Who created this template
    pub created_by: AccountId,
    /// Whether this template is active and usable
    pub is_active: bool,
}

