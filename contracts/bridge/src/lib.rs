#![cfg_attr(not(feature = "std"), no_std)]
#![allow(unexpected_cfgs)]

use ink::prelude::string::String;
use ink::storage::Mapping;
use propchain_traits::*;
#[cfg(not(feature = "std"))]
use scale_info::prelude::vec::Vec;

#[ink::contract]
mod bridge {
    use super::*;
    use propchain_traits::{non_reentrant, ReentrancyError, ReentrancyGuard};

    include!("errors.rs");

    impl From<ReentrancyError> for Error {
        fn from(_: ReentrancyError) -> Self {
            Error::ReentrantCall
        }
    }

    /// Bridge contract for cross-chain property token transfers
    #[ink(storage)]
    pub struct PropertyBridge {
        /// Bridge configuration
        config: BridgeConfig,

        /// Multi-signature bridge requests
        bridge_requests: Mapping<u64, MultisigBridgeRequest>,

        /// Bridge transaction history
        bridge_history: Mapping<AccountId, Vec<BridgeTransaction>>,

        /// Chain-specific information
        chain_info: Mapping<ChainId, ChainBridgeInfo>,

        /// Transaction verification records
        verified_transactions: Mapping<Hash, bool>,

        /// Cross-chain DEX settlement intents tracked by the bridge
        cross_chain_trades: Mapping<u64, CrossChainTradeIntent>,

        /// Per-request cross-chain transaction status tracker. Stores the
        /// per-chain status of every bridge request so callers and indexers
        /// can observe the full lifecycle on both source and destination.
        cross_chain_tx_status: Mapping<u64, CrossChainTxStatus>,

        /// Reverse index from a chain-native transaction hash to the bridge
        /// `request_id`, enabling status lookups by hash from any chain.
        tx_hash_index: Mapping<Hash, u64>,

        /// Bridge operators
        bridge_operators: Vec<AccountId>,

        /// Registered validators for multi-signature cross-chain transactions.
        /// Only accounts in this set may sign bridge requests (issue #203).
        validators: Vec<AccountId>,

        /// Request counter
        request_counter: u64,

        /// Transaction counter
        transaction_counter: u64,

        /// Cross-chain trade settlement counter
        cross_chain_trade_counter: u64,

        /// Admin account
        admin: AccountId,

        /// Registered ECDSA public keys for optional cryptographic signature verification
        operator_public_keys: Mapping<AccountId, [u8; 33]>,

        /// Pending admin key rotation request
        pending_admin_rotation: Option<propchain_traits::KeyRotationRequest>,

        /// Account daily bridge request count for rate limiting
        account_daily_requests: Mapping<AccountId, u64>,

        /// Account last reset day for rate limiting
        account_last_reset_day: Mapping<AccountId, u64>,

        /// Chain daily volume for rate limiting
        chain_daily_volume: Mapping<ChainId, u128>,

        /// Chain last reset day for rate limiting
        chain_last_reset_day: Mapping<ChainId, u64>,

        /// Reentrancy protection
        reentrancy_guard: ReentrancyGuard,
    }

    /// Events for bridge operations
    #[ink(event)]
    pub struct BridgeRequestCreated {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub token_id: TokenId,
        #[ink(topic)]
        pub source_chain: ChainId,
        #[ink(topic)]
        pub destination_chain: ChainId,
        #[ink(topic)]
        pub requester: AccountId,
    }

    #[ink(event)]
    pub struct BridgeRequestSigned {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub signer: AccountId,
        pub signatures_collected: u8,
        pub signatures_required: u8,
    }

    #[ink(event)]
    pub struct BridgeExecuted {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub token_id: TokenId,
        #[ink(topic)]
        pub transaction_hash: Hash,
    }

    #[ink(event)]
    pub struct BridgeFailed {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub token_id: TokenId,
        pub error: String,
    }

    #[ink(event)]
    pub struct BridgeRecovered {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub recovery_action: RecoveryAction,
    }

    /// Emitted when a bridge transaction is atomically rolled back (#201).
    #[ink(event)]
    pub struct BridgeRolledBack {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub token_id: TokenId,
        /// Original sender whose funds are now unlocked.
        pub requester: AccountId,
        /// Human-readable rollback reason for audit trail.
        pub reason: String,
        /// Block number at which the rollback was executed.
        pub rolled_back_at: u32,
    }

    /// Emitted whenever the per-chain status of a cross-chain transaction
    /// changes (creation, leg confirmation, failure, etc.). Off-chain
    /// indexers can subscribe to this event to mirror full bridge state.
    #[ink(event)]
    pub struct CrossChainTxStatusUpdated {
        #[ink(topic)]
        pub request_id: u64,
        #[ink(topic)]
        pub chain_id: ChainId,
        pub status: ChainTxStatus,
        pub overall_status: BridgeOperationStatus,
        pub tx_hash: Option<Hash>,
        pub confirmations: u32,
        pub timestamp: u64,
    }

    impl PropertyBridge {
        /// Creates a new PropertyBridge contract
        #[ink(constructor)]
        pub fn new(
            supported_chains: Vec<ChainId>,
            min_signatures: u8,
            max_signatures: u8,
            default_timeout: u64,
            gas_limit: u64,
        ) -> Self {
            let caller = Self::env().caller();
            let config = BridgeConfig {
                supported_chains: supported_chains.clone(),
                min_signatures_required: min_signatures,
                max_signatures_required: max_signatures,
                default_timeout_blocks: default_timeout,
                gas_limit_per_bridge: gas_limit,
                emergency_pause: false,
                metadata_preservation: true,
                rate_limit_enabled: true,
                max_requests_per_day: 10,
                max_value_per_day: 1_000_000_000_000_000_000,
            };

            // Initialize chain info for supported chains
            let mut bridge = Self {
                config,
                bridge_requests: Mapping::default(),
                bridge_history: Mapping::default(),
                chain_info: Mapping::default(),
                verified_transactions: Mapping::default(),
                cross_chain_trades: Mapping::default(),
                cross_chain_tx_status: Mapping::default(),
                tx_hash_index: Mapping::default(),
                bridge_operators: vec![caller],
                validators: Vec::new(),
                request_counter: 0,
                transaction_counter: 0,
                cross_chain_trade_counter: 0,
                admin: caller,
                operator_public_keys: Mapping::default(),
                pending_admin_rotation: None,
                account_daily_requests: Mapping::default(),
                account_last_reset_day: Mapping::default(),
                chain_daily_volume: Mapping::default(),
                chain_last_reset_day: Mapping::default(),
                reentrancy_guard: ReentrancyGuard::new(),
            };

            // Set up default chain information
            for chain_id in supported_chains {
                let chain_info = ChainBridgeInfo {
                    chain_id,
                    chain_name: format!("Chain-{}", chain_id),
                    bridge_contract_address: None,
                    is_active: true,
                    gas_multiplier: propchain_traits::constants::DEFAULT_GAS_MULTIPLIER,
                    confirmation_blocks: propchain_traits::constants::DEFAULT_CONFIRMATION_BLOCKS,
                    supported_tokens: Vec::new(),
                    chain_daily_limit: 10_000_000_000_000_000_000, // Example large default
                };
                bridge.chain_info.insert(chain_id, &chain_info);
            }

            bridge
        }

        /// Initiates a bridge request with multi-signature requirement
        #[ink(message)]
        pub fn initiate_bridge_multisig(
            &mut self,
            token_id: TokenId,
            destination_chain: ChainId,
            recipient: AccountId,
            required_signatures: u8,
            timeout_blocks: Option<u64>,
            metadata: PropertyMetadata,
        ) -> Result<u64, Error> {
            let caller = self.env().caller();

            // Check if bridge is paused
            if self.config.emergency_pause {
                return Err(Error::BridgePaused);
            }

            // Validate destination chain
            if !self.config.supported_chains.contains(&destination_chain) {
                return Err(Error::InvalidChain);
            }

            // Validate signature requirements
            if required_signatures < self.config.min_signatures_required
                || required_signatures > self.config.max_signatures_required
            {
                return Err(Error::InsufficientSignatures);
            }

            // Check if caller is authorized (token owner or approved operator)
            if !self.is_authorized_for_token(caller, token_id) {
                return Err(Error::Unauthorized);
            }

            // Enforce rate limiting
            // For NFT bridge, we count requests but value is 0 here since NFT value isn't strictly defined by amount.
            self.check_and_update_rate_limits(caller, destination_chain, 0, true)?;

            // Create bridge request
            self.request_counter += 1;
            let request_id = self.request_counter;
            let current_block = u64::from(self.env().block_number());
            let expires_at = timeout_blocks.map(|blocks| current_block + blocks);

            let request = MultisigBridgeRequest {
                request_id,
                token_id,
                source_chain: self.get_current_chain_id(),
                destination_chain,
                sender: caller,
                recipient,
                required_signatures,
                signatures: Vec::new(),
                created_at: current_block,
                expires_at,
                status: BridgeOperationStatus::Pending,
                metadata,
            };

            self.bridge_requests.insert(request_id, &request);

            // Initialize cross-chain transaction status: source leg starts in
            // `Submitted`, destination leg has `NotStarted` until a relayer
            // reports inclusion on the destination chain.
            self.init_cross_chain_status(
                request_id,
                token_id,
                request.source_chain,
                destination_chain,
            );

            self.env().emit_event(BridgeRequestCreated {
                request_id,
                token_id,
                source_chain: request.source_chain,
                destination_chain,
                requester: caller,
            });

            Ok(request_id)
        }

        /// Signs a bridge request
        #[ink(message)]
        pub fn sign_bridge_request(&mut self, request_id: u64, approve: bool) -> Result<(), Error> {
            let caller = self.env().caller();

            // Check if caller is a registered validator (issue #203: only validators may sign)
            if !self.validators.contains(&caller) {
                return Err(Error::Unauthorized);
            }

            let mut request = self
                .bridge_requests
                .get(request_id)
                .ok_or(Error::InvalidRequest)?;

            // Check if request has expired
            if let Some(expires_at) = request.expires_at {
                if u64::from(self.env().block_number()) > expires_at {
                    return Err(Error::RequestExpired);
                }
            }

            // Check if already signed
            if request.signatures.contains(&caller) {
                return Err(Error::AlreadySigned);
            }

            // Add signature
            request.signatures.push(caller);

            // Update status based on approval and signatures collected
            if !approve {
                request.status = BridgeOperationStatus::Failed;
            } else if request.signatures.len() >= request.required_signatures as usize {
                request.status = BridgeOperationStatus::Locked;
            }

            self.bridge_requests.insert(request_id, &request);

            self.env().emit_event(BridgeRequestSigned {
                request_id,
                signer: caller,
                signatures_collected: request.signatures.len() as u8,
                signatures_required: request.required_signatures,
            });

            Ok(())
        }

        /// Register an ECDSA public key for cryptographic signature verification.
        #[ink(message)]
        pub fn register_operator_public_key(&mut self, public_key: [u8; 33]) -> Result<(), Error> {
            let caller = self.env().caller();
            if !self.bridge_operators.contains(&caller) {
                return Err(Error::Unauthorized);
            }
            self.operator_public_keys.insert(caller, &public_key);
            Ok(())
        }

        /// Sign a bridge request with optional ECDSA cryptographic signature verification.
        #[ink(message)]
        pub fn sign_bridge_request_with_signature(
            &mut self,
            request_id: u64,
            approve: bool,
            signed_approval: Option<propchain_traits::SignedApproval>,
        ) -> Result<(), Error> {
            let caller = self.env().caller();

            if let Some(ref approval) = signed_approval {
                let expected_key = self
                    .operator_public_keys
                    .get(caller)
                    .ok_or(Error::Unauthorized)?;
                propchain_traits::crypto::verify_signed_approval(approval, &expected_key)
                    .map_err(|_| Error::Unauthorized)?;

                let expected_hash = propchain_traits::crypto::hash_encoded(&(
                    request_id,
                    approve,
                    caller,
                    self.env().block_number(),
                ));
                if approval.message_hash != <[u8; 32]>::from(expected_hash) {
                    return Err(Error::Unauthorized);
                }
            }

            self.sign_bridge_request(request_id, approve)
        }

        /// Executes a bridge request after collecting required signatures
        #[ink(message)]
        pub fn execute_bridge(&mut self, request_id: u64) -> Result<(), Error> {
            non_reentrant!(self, {
                let caller = self.env().caller();

                // Check if caller is a bridge operator
                if !self.bridge_operators.contains(&caller) {
                    return Err(Error::Unauthorized);
                }

                let mut request = self
                    .bridge_requests
                    .get(request_id)
                    .ok_or(Error::InvalidRequest)?;

                // Check if request is ready for execution
                if request.status != BridgeOperationStatus::Locked {
                    return Err(Error::InvalidRequest);
                }

                // Check if enough signatures are collected
                if request.signatures.len() < request.required_signatures as usize {
                    return Err(Error::InsufficientSignatures);
                }

                // Generate transaction hash
                let transaction_hash = self.generate_transaction_hash(&request);

                // Create bridge transaction record
                self.transaction_counter += 1;
                let transaction = BridgeTransaction {
                    transaction_id: self.transaction_counter,
                    token_id: request.token_id,
                    source_chain: request.source_chain,
                    destination_chain: request.destination_chain,
                    sender: request.sender,
                    recipient: request.recipient,
                    transaction_hash,
                    timestamp: self.env().block_timestamp(),
                    gas_used: self.estimate_gas_usage(&request),
                    status: BridgeOperationStatus::InTransit,
                    metadata: request.metadata.clone(),
                };

                // Update request status
                request.status = BridgeOperationStatus::Completed;
                self.bridge_requests.insert(request_id, &request);

                // Store transaction verification
                self.verified_transactions.insert(transaction_hash, &true);

                // Source leg is now confirmed on the local chain; destination
                // leg moves to `Submitted` (relayer is expected to broadcast
                // the corresponding tx on the destination chain).
                self.advance_cross_chain_status_on_execute(
                    request_id,
                    request.source_chain,
                    request.destination_chain,
                    transaction_hash,
                );

                // Add to bridge history
                let mut history = self.bridge_history.get(request.sender).unwrap_or_default();
                history.push(transaction.clone());
                self.bridge_history.insert(request.sender, &history);

                self.env().emit_event(BridgeExecuted {
                    request_id,
                    token_id: request.token_id,
                    transaction_hash,
                });

                Ok(())
            })
        }

        /// Recovers from a failed bridge operation
        #[ink(message)]
        pub fn recover_failed_bridge(
            &mut self,
            request_id: u64,
            recovery_action: RecoveryAction,
        ) -> Result<(), Error> {
            non_reentrant!(self, {
                let caller = self.env().caller();

                // Only admin can recover failed bridges
                if caller != self.admin {
                    return Err(Error::Unauthorized);
                }

                let mut request = self
                    .bridge_requests
                    .get(request_id)
                    .ok_or(Error::InvalidRequest)?;

                // Check if request is in a failed state
                if !matches!(
                    request.status,
                    BridgeOperationStatus::Failed | BridgeOperationStatus::Expired
                ) {
                    return Err(Error::InvalidRequest);
                }

                // Execute recovery action
                match recovery_action {
                    RecoveryAction::UnlockToken => {
                        // Logic to unlock the token would be implemented here
                        // This would typically call back to the property token contract
                    }
                    RecoveryAction::RefundGas => {
                        // Logic to refund gas costs would be implemented here
                    }
                    RecoveryAction::RetryBridge => {
                        // Reset request to pending for retry
                        request.status = BridgeOperationStatus::Pending;
                        request.signatures.clear();
                    }
                    RecoveryAction::CancelBridge => {
                        // Mark as cancelled
                        request.status = BridgeOperationStatus::Failed;
                    }
                }

                self.bridge_requests.insert(request_id, &request);

                self.env().emit_event(BridgeRecovered {
                    request_id,
                    recovery_action,
                });

                Ok(())
            })
        }

        // ── #201: Transaction rollback mechanism ─────────────────────────────────

        /// Rollback a failed or expired bridge transaction (#201).
        ///
        /// This provides a structured, atomic rollback path for bridge requests that
        /// got stuck in `Failed`, `Expired`, or `InTransit` states. Unlike the more
        /// general `recover_failed_bridge`, a rollback:
        ///
        ///   1. Resets the request to `Recovering` (prevents concurrent rollbacks).
        ///   2. Clears all collected signatures so the request cannot be accidentally
        ///      re-executed.
        ///   3. Marks the request as `Failed` (terminal rollback state).
        ///   4. Records the rollback block number for audit.
        ///   5. Emits a `BridgeRolledBack` event for off-chain indexers.
        ///
        /// Only the bridge admin may trigger a rollback.
        #[ink(message)]
        pub fn rollback_bridge_transaction(
            &mut self,
            request_id: u64,
            reason: String,
        ) -> Result<(), Error> {
            non_reentrant!(self, {
                let caller = self.env().caller();
                if caller != self.admin {
                    return Err(Error::Unauthorized);
                }

                let mut request = self
                    .bridge_requests
                    .get(request_id)
                    .ok_or(Error::InvalidRequest)?;

                // Only rollback requests that are in a non-terminal, non-completed state
                match request.status {
                    BridgeOperationStatus::Completed => {
                        // Completed requests cannot be rolled back — funds already moved
                        return Err(Error::InvalidRequest);
                    }
                    BridgeOperationStatus::None => {
                        return Err(Error::InvalidRequest);
                    }
                    _ => {}
                }

                // Step 1: mark as Recovering to prevent concurrent rollbacks
                request.status = BridgeOperationStatus::Recovering;
                self.bridge_requests.insert(request_id, &request);

                // Step 2: clear signatures so the request cannot be re-executed
                request.signatures.clear();

                // Step 3: mark as Failed (terminal rollback state)
                request.status = BridgeOperationStatus::Failed;
                self.bridge_requests.insert(request_id, &request);

                // Step 4 + 5: emit structured rollback event for indexers
                self.env().emit_event(BridgeRolledBack {
                    request_id,
                    token_id: request.token_id,
                    requester: request.sender,
                    reason,
                    rolled_back_at: self.env().block_number(),
                });

                // Mark both source and destination legs as Failed in the
                // cross-chain tracker so external observers see a terminal
                // state instead of stale in-flight statuses.
                self.fail_cross_chain_status(
                    request_id,
                    request.source_chain,
                    request.destination_chain,
                );

                Ok(())
            })
        }

        /// Gets gas estimation for a bridge operation
        #[ink(message)]
        pub fn estimate_bridge_gas(
            &self,
            _token_id: TokenId,
            destination_chain: ChainId,
        ) -> Result<u64, Error> {
            let chain_info = self
                .chain_info
                .get(destination_chain)
                .ok_or(Error::InvalidChain)?;
            if !chain_info.is_active {
                return Err(Error::InvalidChain);
            }

            let base_gas = propchain_traits::constants::BRIDGE_BASE_GAS;
            let multiplier = u64::from(chain_info.gas_multiplier);
            let confirmation_blocks = u64::from(chain_info.confirmation_blocks);
            let adjusted_base = base_gas.saturating_mul(multiplier) / 100;
            let confirmation_overhead = adjusted_base.saturating_mul(confirmation_blocks) / 100;
            let estimated = adjusted_base.saturating_add(confirmation_overhead);

            Ok(estimated.min(self.config.gas_limit_per_bridge))
        }

        /// Monitors bridge status
        #[ink(message)]
        pub fn monitor_bridge_status(&self, request_id: u64) -> Option<BridgeMonitoringInfo> {
            let request = self.bridge_requests.get(request_id)?;

            Some(BridgeMonitoringInfo {
                bridge_request_id: request.request_id,
                token_id: request.token_id,
                source_chain: request.source_chain,
                destination_chain: request.destination_chain,
                status: request.status,
                created_at: request.created_at,
                expires_at: request.expires_at,
                signatures_collected: request.signatures.len() as u8,
                signatures_required: request.required_signatures,
                error_message: None,
            })
        }

        /// Verifies a bridge transaction
        #[ink(message)]
        pub fn verify_bridge_transaction(
            &self,
            transaction_hash: Hash,
            _source_chain: ChainId,
        ) -> bool {
            self.verified_transactions
                .get(transaction_hash)
                .unwrap_or(false)
        }

        /// Gets bridge history for an account
        #[ink(message)]
        pub fn get_bridge_history(&self, account: AccountId) -> Vec<BridgeTransaction> {
            self.bridge_history.get(account).unwrap_or_default()
        }

        /// Quotes bridge fees for a DEX settlement.
        #[ink(message)]
        pub fn quote_cross_chain_trade(
            &self,
            destination_chain: ChainId,
            amount_in: u128,
        ) -> Result<BridgeFeeQuote, Error> {
            let chain_info = self
                .chain_info
                .get(destination_chain)
                .ok_or(Error::InvalidChain)?;
            let gas_estimate = self.estimate_bridge_gas(0, destination_chain)?;
            let protocol_fee = amount_in / 200;
            // Convert gas usage into an amount-based fee so totals stay in token units.
            let gas_fee = if self.config.gas_limit_per_bridge == 0 {
                0
            } else {
                let gas_ratio_bps = (u128::from(gas_estimate).saturating_mul(10_000))
                    / u128::from(self.config.gas_limit_per_bridge);
                let chain_risk_bps = u128::from(chain_info.confirmation_blocks).saturating_mul(10);
                let adjusted_bps = gas_ratio_bps.saturating_add(chain_risk_bps).min(2_500);
                amount_in.saturating_mul(adjusted_bps) / 10_000
            };
            Ok(BridgeFeeQuote {
                destination_chain,
                gas_estimate,
                protocol_fee,
                total_fee: protocol_fee.saturating_add(gas_fee),
            })
        }

        /// Registers a cross-chain DEX trade intent on the bridge.
        #[ink(message)]
        pub fn register_cross_chain_trade(
            &mut self,
            pair_id: u64,
            order_id: Option<u64>,
            destination_chain: ChainId,
            recipient: AccountId,
            amount_in: u128,
            min_amount_out: u128,
        ) -> Result<u64, Error> {
            if self.config.emergency_pause {
                return Err(Error::BridgePaused);
            }
            if !self.config.supported_chains.contains(&destination_chain) {
                return Err(Error::InvalidChain);
            }

            // Enforce rate limiting
            // For cross-chain trades, we track the volume (amount_in) but don't count it as an NFT request.
            self.check_and_update_rate_limits(
                self.env().caller(),
                destination_chain,
                amount_in,
                false,
            )?;

            self.cross_chain_trade_counter += 1;
            let trade_id = self.cross_chain_trade_counter;
            let quote = self.quote_cross_chain_trade(destination_chain, amount_in)?;
            let intent = CrossChainTradeIntent {
                trade_id,
                pair_id,
                order_id,
                source_chain: self.get_current_chain_id(),
                destination_chain,
                trader: self.env().caller(),
                recipient,
                amount_in,
                min_amount_out,
                bridge_request_id: None,
                bridge_fee_quote: quote,
                status: CrossChainTradeStatus::Pending,
                created_at: self.env().block_timestamp(),
            };
            self.cross_chain_trades.insert(trade_id, &intent);
            Ok(trade_id)
        }

        /// Attaches a bridge request to a pending cross-chain trade.
        #[ink(message)]
        pub fn attach_bridge_request_to_trade(
            &mut self,
            trade_id: u64,
            bridge_request_id: u64,
        ) -> Result<(), Error> {
            let caller = self.env().caller();
            let mut trade = self
                .cross_chain_trades
                .get(trade_id)
                .ok_or(Error::InvalidRequest)?;
            if caller != trade.trader && caller != self.admin {
                return Err(Error::Unauthorized);
            }
            trade.bridge_request_id = Some(bridge_request_id);
            trade.status = CrossChainTradeStatus::BridgeRequested;
            self.cross_chain_trades.insert(trade_id, &trade);
            Ok(())
        }

        /// Marks a cross-chain trade settlement as complete.
        #[ink(message)]
        pub fn settle_cross_chain_trade(&mut self, trade_id: u64) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin && !self.bridge_operators.contains(&caller) {
                return Err(Error::Unauthorized);
            }
            let mut trade = self
                .cross_chain_trades
                .get(trade_id)
                .ok_or(Error::InvalidRequest)?;
            trade.status = CrossChainTradeStatus::Settled;
            self.cross_chain_trades.insert(trade_id, &trade);
            Ok(())
        }

        /// Gets a cross-chain trade settlement intent.
        #[ink(message)]
        pub fn get_cross_chain_trade(&self, trade_id: u64) -> Option<CrossChainTradeIntent> {
            self.cross_chain_trades.get(trade_id)
        }

        /// Adds a bridge operator
        #[ink(message)]
        pub fn add_bridge_operator(&mut self, operator: AccountId) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            if !self.bridge_operators.contains(&operator) {
                self.bridge_operators.push(operator);
            }

            Ok(())
        }

        /// Removes a bridge operator
        #[ink(message)]
        pub fn remove_bridge_operator(&mut self, operator: AccountId) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            self.bridge_operators.retain(|op| op != &operator);
            Ok(())
        }

        /// Checks if an account is a bridge operator
        #[ink(message)]
        pub fn is_bridge_operator(&self, account: AccountId) -> bool {
            self.bridge_operators.contains(&account)
        }

        /// Gets all bridge operators
        #[ink(message)]
        pub fn get_bridge_operators(&self) -> Vec<AccountId> {
            self.bridge_operators.clone()
        }

        /// Adds a validator (admin only). Only validators may sign bridge requests (issue #203).
        #[ink(message)]
        pub fn add_validator(&mut self, validator: AccountId) -> Result<(), Error> {
            if self.env().caller() != self.admin {
                return Err(Error::Unauthorized);
            }
            if !self.validators.contains(&validator) {
                self.validators.push(validator);
            }
            Ok(())
        }

        /// Removes a validator (admin only).
        #[ink(message)]
        pub fn remove_validator(&mut self, validator: AccountId) -> Result<(), Error> {
            if self.env().caller() != self.admin {
                return Err(Error::Unauthorized);
            }
            self.validators.retain(|v| v != &validator);
            Ok(())
        }

        /// Returns all registered validators.
        #[ink(message)]
        pub fn get_validators(&self) -> Vec<AccountId> {
            self.validators.clone()
        }

        /// Returns whether an account is a registered validator.
        #[ink(message)]
        pub fn is_validator(&self, account: AccountId) -> bool {
            self.validators.contains(&account)
        }
        /// Updates bridge configuration (admin only)
        #[ink(message)]
        pub fn update_config(&mut self, config: BridgeConfig) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            self.config = config;
            Ok(())
        }

        /// Gets current bridge configuration
        #[ink(message)]
        pub fn get_config(&self) -> BridgeConfig {
            self.config.clone()
        }

        /// Pauses or unpauses the bridge (admin only)
        #[ink(message)]
        pub fn set_emergency_pause(&mut self, paused: bool) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            self.config.emergency_pause = paused;
            Ok(())
        }

        /// Gets chain information
        #[ink(message)]
        pub fn get_chain_info(&self, chain_id: ChainId) -> Option<ChainBridgeInfo> {
            self.chain_info.get(chain_id)
        }

        /// Updates chain information (admin only)
        #[ink(message)]
        pub fn update_chain_info(
            &mut self,
            chain_id: ChainId,
            info: ChainBridgeInfo,
        ) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            self.chain_info.insert(chain_id, &info);
            Ok(())
        }

        /// Request a two-step admin rotation with cooldown.
        #[ink(message)]
        pub fn request_admin_rotation(&mut self, new_admin: AccountId) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin {
                return Err(Error::Unauthorized);
            }

            let block = self.env().block_number();
            let effective_at =
                block.saturating_add(propchain_traits::constants::KEY_ROTATION_COOLDOWN_BLOCKS);

            self.pending_admin_rotation = Some(propchain_traits::KeyRotationRequest {
                old_account: caller,
                new_account: new_admin,
                requested_at: block,
                effective_at,
                confirmed: false,
            });

            Ok(())
        }

        /// Confirm a pending admin rotation after cooldown.
        #[ink(message)]
        pub fn confirm_admin_rotation(&mut self) -> Result<(), Error> {
            let caller = self.env().caller();
            let block = self.env().block_number();

            let request = self
                .pending_admin_rotation
                .as_ref()
                .ok_or(Error::InvalidRequest)?;

            if request.new_account != caller {
                return Err(Error::Unauthorized);
            }
            if block < request.effective_at {
                return Err(Error::InvalidRequest);
            }
            let expiry = request
                .effective_at
                .saturating_add(propchain_traits::constants::KEY_ROTATION_EXPIRY_BLOCKS);
            if block > expiry {
                self.pending_admin_rotation = None;
                return Err(Error::RequestExpired);
            }

            self.admin = caller;
            self.pending_admin_rotation = None;
            Ok(())
        }

        /// Cancel a pending admin rotation.
        #[ink(message)]
        pub fn cancel_admin_rotation(&mut self) -> Result<(), Error> {
            let caller = self.env().caller();
            let request = self
                .pending_admin_rotation
                .as_ref()
                .ok_or(Error::InvalidRequest)?;

            if caller != request.old_account && caller != request.new_account {
                return Err(Error::Unauthorized);
            }

            self.pending_admin_rotation = None;
            Ok(())
        }

        // ── Cross-chain transaction status tracking (TASK 1) ───────────────

        /// Update the per-chain status of an in-flight cross-chain transaction.
        ///
        /// Authorized callers (admin or any registered bridge operator) can
        /// post status reports for either the source or destination chain as
        /// the transaction progresses. The bridge contract itself records
        /// updates for the source chain on initiate/execute/rollback; this
        /// message is primarily intended for relayers reporting on the
        /// destination chain.
        ///
        /// `chain_id` must match either the source or destination chain of
        /// the request. The latest snapshot replaces the previous one for
        /// that leg, and a copy is appended to `history` for audit.
        #[ink(message)]
        pub fn update_chain_tx_status(
            &mut self,
            request_id: u64,
            chain_id: ChainId,
            status: ChainTxStatus,
            tx_hash: Option<Hash>,
            block_number: u64,
            confirmations: u32,
            error_message: Option<String>,
        ) -> Result<(), Error> {
            let caller = self.env().caller();
            if caller != self.admin && !self.bridge_operators.contains(&caller) {
                return Err(Error::Unauthorized);
            }

            let mut tracker = self
                .cross_chain_tx_status
                .get(request_id)
                .ok_or(Error::TransactionNotFound)?;

            if chain_id != tracker.source_chain && chain_id != tracker.destination_chain {
                return Err(Error::InvalidChain);
            }

            // Reject obviously invalid transitions (e.g. moving a Confirmed
            // leg back to NotStarted/Submitted). Failed → * is allowed only
            // via the recovery flow, not via status reports.
            let current = if chain_id == tracker.source_chain {
                tracker.source_status.status
            } else {
                tracker.destination_status.status
            };
            if !is_valid_chain_status_transition(current, status) {
                return Err(Error::InvalidStatusTransition);
            }

            let timestamp = self.env().block_timestamp();
            let update = ChainStatusUpdate {
                chain_id,
                status,
                tx_hash,
                block_number,
                timestamp,
                confirmations,
                error_message,
            };

            if chain_id == tracker.source_chain {
                tracker.source_status = update.clone();
            } else {
                tracker.destination_status = update.clone();
            }
            tracker.history.push(update.clone());
            tracker.last_updated = timestamp;
            tracker.overall_status = compute_overall_status(
                tracker.source_status.status,
                tracker.destination_status.status,
            );

            // Index by tx_hash so callers can look up status from any chain.
            if let Some(hash) = tx_hash {
                self.tx_hash_index.insert(hash, &request_id);
            }

            self.cross_chain_tx_status.insert(request_id, &tracker);

            self.env().emit_event(CrossChainTxStatusUpdated {
                request_id,
                chain_id,
                status,
                overall_status: tracker.overall_status.clone(),
                tx_hash,
                confirmations,
                timestamp,
            });

            Ok(())
        }

        /// Convenience message for relayers to mark the destination leg as
        /// `Confirmed` once the foreign-chain transaction has reached the
        /// configured confirmation depth.
        #[ink(message)]
        pub fn confirm_destination_delivery(
            &mut self,
            request_id: u64,
            destination_tx_hash: Hash,
            block_number: u64,
            confirmations: u32,
        ) -> Result<(), Error> {
            let tracker = self
                .cross_chain_tx_status
                .get(request_id)
                .ok_or(Error::TransactionNotFound)?;
            let destination_chain = tracker.destination_chain;
            self.update_chain_tx_status(
                request_id,
                destination_chain,
                ChainTxStatus::Confirmed,
                Some(destination_tx_hash),
                block_number,
                confirmations,
                None,
            )
        }

        /// Returns the full cross-chain transaction status, including the
        /// latest snapshot on each chain plus the chronological update log.
        #[ink(message)]
        pub fn get_cross_chain_tx_status(
            &self,
            request_id: u64,
        ) -> Option<CrossChainTxStatus> {
            self.cross_chain_tx_status.get(request_id)
        }

        /// Returns the latest status snapshot for a specific chain leg of a
        /// given request. `None` if the request is unknown or the supplied
        /// `chain_id` is neither the source nor destination of the request.
        #[ink(message)]
        pub fn get_chain_status(
            &self,
            request_id: u64,
            chain_id: ChainId,
        ) -> Option<ChainStatusUpdate> {
            let tracker = self.cross_chain_tx_status.get(request_id)?;
            if chain_id == tracker.source_chain {
                Some(tracker.source_status)
            } else if chain_id == tracker.destination_chain {
                Some(tracker.destination_status)
            } else {
                None
            }
        }

        /// Look up a cross-chain transaction status by any chain-native
        /// transaction hash that has been reported to the bridge.
        #[ink(message)]
        pub fn get_tx_status_by_hash(&self, tx_hash: Hash) -> Option<CrossChainTxStatus> {
            let request_id = self.tx_hash_index.get(tx_hash)?;
            self.cross_chain_tx_status.get(request_id)
        }

        /// Returns the full chronological per-chain update history for a
        /// request. Useful for off-chain audit and dashboards.
        #[ink(message)]
        pub fn get_tx_status_history(&self, request_id: u64) -> Vec<ChainStatusUpdate> {
            self.cross_chain_tx_status
                .get(request_id)
                .map(|t| t.history)
                .unwrap_or_default()
        }

        // Helper functions

        fn is_authorized_for_token(&self, _account: AccountId, _token_id: TokenId) -> bool {
            // This would typically check with the property token contract
            // For now, we'll assume any account can initiate a bridge
            true
        }

        fn get_current_chain_id(&self) -> ChainId {
            // This should return the current chain ID
            // For now, we'll use a default value
            1
        }

        fn generate_transaction_hash(&self, request: &MultisigBridgeRequest) -> Hash {
            let data = (
                request.request_id,
                request.token_id,
                request.source_chain,
                request.destination_chain,
                request.sender,
                request.recipient,
                self.env().block_timestamp(),
            );
            propchain_traits::crypto::hash_encoded(&data)
        }

        fn estimate_gas_usage(&self, request: &MultisigBridgeRequest) -> u64 {
            // Estimate gas usage based on request complexity
            let base_gas = 100000; // Base gas for bridge operation
            let metadata_gas = request.metadata.legal_description.len() as u64 * 100; // Gas for metadata
            base_gas + metadata_gas
        }

        fn check_and_update_rate_limits(
            &mut self,
            account: AccountId,
            destination_chain: ChainId,
            amount: u128,
            is_nft: bool,
        ) -> Result<(), Error> {
            if !self.config.rate_limit_enabled {
                return Ok(());
            }

            let current_day = self.env().block_timestamp() / 86_400_000;

            if is_nft {
                let last_reset = self.account_last_reset_day.get(account).unwrap_or(0);
                let mut daily_requests = self.account_daily_requests.get(account).unwrap_or(0);

                if last_reset < current_day {
                    daily_requests = 0;
                    self.account_last_reset_day.insert(account, &current_day);
                }

                if daily_requests >= self.config.max_requests_per_day {
                    return Err(Error::RateLimitExceeded);
                }

                self.account_daily_requests
                    .insert(account, &(daily_requests + 1));
            }

            if amount > 0 {
                let chain_info = self
                    .chain_info
                    .get(destination_chain)
                    .ok_or(Error::InvalidChain)?;
                let last_chain_reset = self
                    .chain_last_reset_day
                    .get(destination_chain)
                    .unwrap_or(0);
                let mut chain_volume = self.chain_daily_volume.get(destination_chain).unwrap_or(0);

                if last_chain_reset < current_day {
                    chain_volume = 0;
                    self.chain_last_reset_day
                        .insert(destination_chain, &current_day);
                }

                if chain_volume.saturating_add(amount) > chain_info.chain_daily_limit {
                    return Err(Error::RateLimitExceeded);
                }

                self.chain_daily_volume
                    .insert(destination_chain, &(chain_volume + amount));
            }

            Ok(())
        }

        // ── Cross-chain status helper methods ──────────────────────────

        /// Initialize the cross-chain transaction tracker for a new request.
        /// The source leg is recorded as `Submitted` (the on-chain initiation
        /// is itself the source-chain submission); the destination leg is
        /// `NotStarted` until a relayer reports activity on that chain.
        fn init_cross_chain_status(
            &mut self,
            request_id: u64,
            token_id: TokenId,
            source_chain: ChainId,
            destination_chain: ChainId,
        ) {
            let timestamp = self.env().block_timestamp();
            let block_number = u64::from(self.env().block_number());

            let source = ChainStatusUpdate {
                chain_id: source_chain,
                status: ChainTxStatus::Submitted,
                tx_hash: None,
                block_number,
                timestamp,
                confirmations: 0,
                error_message: None,
            };
            let destination = ChainStatusUpdate {
                chain_id: destination_chain,
                status: ChainTxStatus::NotStarted,
                tx_hash: None,
                block_number: 0,
                timestamp,
                confirmations: 0,
                error_message: None,
            };

            let mut history = Vec::new();
            history.push(source.clone());

            let tracker = CrossChainTxStatus {
                request_id,
                token_id,
                source_chain,
                destination_chain,
                source_status: source.clone(),
                destination_status: destination,
                overall_status: BridgeOperationStatus::Pending,
                history,
                last_updated: timestamp,
            };
            self.cross_chain_tx_status.insert(request_id, &tracker);

            self.env().emit_event(CrossChainTxStatusUpdated {
                request_id,
                chain_id: source_chain,
                status: ChainTxStatus::Submitted,
                overall_status: BridgeOperationStatus::Pending,
                tx_hash: None,
                confirmations: 0,
                timestamp,
            });
        }

        /// Advance the tracker on successful `execute_bridge`: source leg is
        /// `Confirmed` (with the generated tx_hash), destination leg moves
        /// to `Submitted` awaiting relayer confirmation.
        fn advance_cross_chain_status_on_execute(
            &mut self,
            request_id: u64,
            source_chain: ChainId,
            destination_chain: ChainId,
            tx_hash: Hash,
        ) {
            let mut tracker = match self.cross_chain_tx_status.get(request_id) {
                Some(t) => t,
                // Defensive: should always exist (init on initiate), but if
                // a record is somehow missing we silently no-op rather than
                // panic the execute flow.
                None => return,
            };
            let timestamp = self.env().block_timestamp();
            let block_number = u64::from(self.env().block_number());

            let source_update = ChainStatusUpdate {
                chain_id: source_chain,
                status: ChainTxStatus::Confirmed,
                tx_hash: Some(tx_hash),
                block_number,
                timestamp,
                confirmations: 1,
                error_message: None,
            };
            let destination_update = ChainStatusUpdate {
                chain_id: destination_chain,
                status: ChainTxStatus::Submitted,
                tx_hash: None,
                block_number: 0,
                timestamp,
                confirmations: 0,
                error_message: None,
            };

            tracker.source_status = source_update.clone();
            tracker.destination_status = destination_update.clone();
            tracker.history.push(source_update);
            tracker.history.push(destination_update);
            tracker.last_updated = timestamp;
            tracker.overall_status = BridgeOperationStatus::InTransit;

            // Record the source-chain tx hash in the reverse index.
            self.tx_hash_index.insert(tx_hash, &request_id);

            self.cross_chain_tx_status.insert(request_id, &tracker);

            self.env().emit_event(CrossChainTxStatusUpdated {
                request_id,
                chain_id: source_chain,
                status: ChainTxStatus::Confirmed,
                overall_status: BridgeOperationStatus::InTransit,
                tx_hash: Some(tx_hash),
                confirmations: 1,
                timestamp,
            });
        }

        /// Mark both legs as failed on rollback so dashboards observe a
        /// terminal state instead of a stale in-flight status.
        fn fail_cross_chain_status(
            &mut self,
            request_id: u64,
            source_chain: ChainId,
            destination_chain: ChainId,
        ) {
            let mut tracker = match self.cross_chain_tx_status.get(request_id) {
                Some(t) => t,
                None => return,
            };
            let timestamp = self.env().block_timestamp();
            let block_number = u64::from(self.env().block_number());

            // Only mark a leg as Failed if it isn't already in a terminal
            // success state (Confirmed). This preserves accurate per-chain
            // history when only one side failed.
            if tracker.source_status.status != ChainTxStatus::Confirmed {
                let upd = ChainStatusUpdate {
                    chain_id: source_chain,
                    status: ChainTxStatus::Failed,
                    tx_hash: tracker.source_status.tx_hash,
                    block_number,
                    timestamp,
                    confirmations: tracker.source_status.confirmations,
                    error_message: Some(String::from("Bridge rollback")),
                };
                tracker.source_status = upd.clone();
                tracker.history.push(upd);
            }
            if tracker.destination_status.status != ChainTxStatus::Confirmed {
                let upd = ChainStatusUpdate {
                    chain_id: destination_chain,
                    status: ChainTxStatus::Failed,
                    tx_hash: tracker.destination_status.tx_hash,
                    block_number,
                    timestamp,
                    confirmations: tracker.destination_status.confirmations,
                    error_message: Some(String::from("Bridge rollback")),
                };
                tracker.destination_status = upd.clone();
                tracker.history.push(upd);
            }
            tracker.last_updated = timestamp;
            tracker.overall_status = BridgeOperationStatus::Failed;
            self.cross_chain_tx_status.insert(request_id, &tracker);

            self.env().emit_event(CrossChainTxStatusUpdated {
                request_id,
                chain_id: source_chain,
                status: ChainTxStatus::Failed,
                overall_status: BridgeOperationStatus::Failed,
                tx_hash: None,
                confirmations: 0,
                timestamp,
            });
        }
    }

    /// Free helper: validate per-chain status transitions.
    ///
    /// Allowed transitions (forward progress only):
    ///   NotStarted → {Submitted, Failed}
    ///   Submitted  → {Submitted, Confirming, Confirmed, Failed}
    ///   Confirming → {Confirming, Confirmed, Failed}
    ///   Confirmed  → {Confirmed}            (terminal-success; only confirmation count may change)
    ///   Failed     → {Failed}               (terminal-failure)
    fn is_valid_chain_status_transition(from: ChainTxStatus, to: ChainTxStatus) -> bool {
        use ChainTxStatus::*;
        match (from, to) {
            (NotStarted, Submitted)
            | (NotStarted, Failed)
            | (Submitted, Submitted)
            | (Submitted, Confirming)
            | (Submitted, Confirmed)
            | (Submitted, Failed)
            | (Confirming, Confirming)
            | (Confirming, Confirmed)
            | (Confirming, Failed)
            | (Confirmed, Confirmed)
            | (Failed, Failed) => true,
            _ => false,
        }
    }

    /// Free helper: derive the aggregated `BridgeOperationStatus` from the
    /// individual per-chain statuses.
    fn compute_overall_status(
        source: ChainTxStatus,
        destination: ChainTxStatus,
    ) -> BridgeOperationStatus {
        use ChainTxStatus::*;
        match (source, destination) {
            (Failed, _) | (_, Failed) => BridgeOperationStatus::Failed,
            (Confirmed, Confirmed) => BridgeOperationStatus::Completed,
            (Confirmed, _) => BridgeOperationStatus::InTransit,
            (Submitted, NotStarted) | (Confirming, NotStarted) => BridgeOperationStatus::Pending,
            _ => BridgeOperationStatus::InTransit,
        }
    }
}
