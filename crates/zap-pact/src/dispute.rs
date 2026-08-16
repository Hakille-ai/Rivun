//! Multi-Party Agent Pact Escrow, Dispute Mediation, and Slashing Engine.
//!
//! Provides conditional resource locking, multi-signature threshold releases,
//! deterministic dispute resolution, and timeout slashing across autonomous agents.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DisputeError {
    #[error("pact {0} not found")]
    PactNotFound(Uuid),
    #[error("invalid pact state transition from {from:?} to {to:?}")]
    InvalidStateTransition { from: PactState, to: PactState },
    #[error("unauthorized caller {0}")]
    Unauthorized(Uuid),
    #[error("arbitration threshold not met: {got}/{required}")]
    ArbitrationThresholdNotMet { got: usize, required: usize },
    #[error("pact has not expired yet: now={now_micros}, expires={timeout_micros}")]
    PactNotExpired {
        now_micros: u64,
        timeout_micros: u64,
    },
    #[error("arbitrator {0} already voted on dispute for pact {1}")]
    DuplicateArbitrationVote(Uuid, Uuid),
}

/// Lifecycle states of an Escrow Agent Pact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PactState {
    Locked,
    Settled,
    Disputed,
    Slashed,
}

/// Arbitration Ruling Outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RulingOutcome {
    ReleaseToRecipient,
    SlashRefundToSender,
    SplitEqual,
}

/// Multi-Party Conditional Escrow Pact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EscrowPact {
    pub pact_id: Uuid,
    pub sender_node_id: Uuid,
    pub recipient_node_id: Uuid,
    pub escrow_units: u64,
    pub action_commitment_hash: String,
    pub timeout_micros: u64,
    pub arbitration_nodes: Vec<Uuid>,
    pub arbitration_threshold: usize,
    pub state: PactState,
    pub settled_recipient_units: u64,
    pub refunded_sender_units: u64,
}

/// Evidence submitted for a dispute.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisputeEvidence {
    pub evidence_id: Uuid,
    pub submitter_node_id: Uuid,
    pub violation_code: String,
    pub payload_hash: String,
    pub signature: String,
}

/// Dispute Case Record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisputeCase {
    pub pact_id: Uuid,
    pub opened_by: Uuid,
    pub opened_at_micros: u64,
    pub evidence: Vec<DisputeEvidence>,
    pub votes: HashMap<Uuid, (RulingOutcome, String)>, // arbitrator_id -> (outcome, sig)
    pub final_ruling: Option<RulingOutcome>,
}

/// Central Dispute Resolution Engine.
#[derive(Clone, Debug, Default)]
pub struct DisputeEngine {
    pub pacts: HashMap<Uuid, EscrowPact>,
    pub disputes: HashMap<Uuid, DisputeCase>,
}

impl DisputeEngine {
    pub fn new() -> Self {
        Self {
            pacts: HashMap::new(),
            disputes: HashMap::new(),
        }
    }

    /// Lock resources in an escrow pact.
    #[allow(clippy::too_many_arguments)]
    pub fn create_escrow_pact(
        &mut self,
        pact_id: Uuid,
        sender_node_id: Uuid,
        recipient_node_id: Uuid,
        escrow_units: u64,
        action_commitment_hash: impl Into<String>,
        timeout_micros: u64,
        arbitration_nodes: Vec<Uuid>,
        arbitration_threshold: usize,
    ) -> &EscrowPact {
        let pact = EscrowPact {
            pact_id,
            sender_node_id,
            recipient_node_id,
            escrow_units,
            action_commitment_hash: action_commitment_hash.into(),
            timeout_micros,
            arbitration_nodes,
            arbitration_threshold,
            state: PactState::Locked,
            settled_recipient_units: 0,
            refunded_sender_units: 0,
        };
        self.pacts.insert(pact_id, pact);
        self.pacts.get(&pact_id).unwrap()
    }

    /// Settle pact normally upon verified execution receipt.
    pub fn settle_normal(&mut self, pact_id: Uuid, caller: Uuid) -> Result<(), DisputeError> {
        let pact = self
            .pacts
            .get_mut(&pact_id)
            .ok_or(DisputeError::PactNotFound(pact_id))?;

        if pact.state != PactState::Locked {
            return Err(DisputeError::InvalidStateTransition {
                from: pact.state,
                to: PactState::Settled,
            });
        }

        if caller != pact.sender_node_id && caller != pact.recipient_node_id {
            return Err(DisputeError::Unauthorized(caller));
        }

        pact.state = PactState::Settled;
        pact.settled_recipient_units = pact.escrow_units;
        pact.refunded_sender_units = 0;
        Ok(())
    }

    /// Slash and refund on timeout expiration.
    pub fn execute_timeout_slash(
        &mut self,
        pact_id: Uuid,
        now_micros: u64,
    ) -> Result<(), DisputeError> {
        let pact = self
            .pacts
            .get_mut(&pact_id)
            .ok_or(DisputeError::PactNotFound(pact_id))?;

        if pact.state != PactState::Locked {
            return Err(DisputeError::InvalidStateTransition {
                from: pact.state,
                to: PactState::Slashed,
            });
        }

        if now_micros <= pact.timeout_micros {
            return Err(DisputeError::PactNotExpired {
                now_micros,
                timeout_micros: pact.timeout_micros,
            });
        }

        pact.state = PactState::Slashed;
        pact.settled_recipient_units = 0;
        pact.refunded_sender_units = pact.escrow_units;
        Ok(())
    }

    /// Open a formal dispute for mediation.
    pub fn open_dispute(
        &mut self,
        pact_id: Uuid,
        opened_by: Uuid,
        initial_evidence: DisputeEvidence,
        now_micros: u64,
    ) -> Result<(), DisputeError> {
        let pact = self
            .pacts
            .get_mut(&pact_id)
            .ok_or(DisputeError::PactNotFound(pact_id))?;

        if pact.state != PactState::Locked {
            return Err(DisputeError::InvalidStateTransition {
                from: pact.state,
                to: PactState::Disputed,
            });
        }

        pact.state = PactState::Disputed;

        let dispute = DisputeCase {
            pact_id,
            opened_by,
            opened_at_micros: now_micros,
            evidence: vec![initial_evidence],
            votes: HashMap::new(),
            final_ruling: None,
        };

        self.disputes.insert(pact_id, dispute);
        Ok(())
    }

    /// Cast an arbitrator vote on an active dispute.
    pub fn submit_arbitration_vote(
        &mut self,
        pact_id: Uuid,
        arbitrator_id: Uuid,
        outcome: RulingOutcome,
        signature: impl Into<String>,
    ) -> Result<Option<RulingOutcome>, DisputeError> {
        let pact = self
            .pacts
            .get(&pact_id)
            .ok_or(DisputeError::PactNotFound(pact_id))?;

        if !pact.arbitration_nodes.contains(&arbitrator_id) {
            return Err(DisputeError::Unauthorized(arbitrator_id));
        }

        let threshold = pact.arbitration_threshold;
        let total_escrow = pact.escrow_units;

        let dispute = self
            .disputes
            .get_mut(&pact_id)
            .ok_or(DisputeError::PactNotFound(pact_id))?;

        if dispute.votes.contains_key(&arbitrator_id) {
            return Err(DisputeError::DuplicateArbitrationVote(
                arbitrator_id,
                pact_id,
            ));
        }

        dispute
            .votes
            .insert(arbitrator_id, (outcome, signature.into()));

        // Count votes per outcome
        let mut outcome_counts: HashMap<RulingOutcome, usize> = HashMap::new();
        for (v_outcome, _) in dispute.votes.values() {
            *outcome_counts.entry(*v_outcome).or_insert(0) += 1;
        }

        if let Some((&winning_outcome, _)) = outcome_counts.iter().find(|(_, c)| **c >= threshold) {
            dispute.final_ruling = Some(winning_outcome);

            // Apply ruling to pact
            if let Some(pact_mut) = self.pacts.get_mut(&pact_id) {
                match winning_outcome {
                    RulingOutcome::ReleaseToRecipient => {
                        pact_mut.state = PactState::Settled;
                        pact_mut.settled_recipient_units = total_escrow;
                        pact_mut.refunded_sender_units = 0;
                    }
                    RulingOutcome::SlashRefundToSender => {
                        pact_mut.state = PactState::Slashed;
                        pact_mut.settled_recipient_units = 0;
                        pact_mut.refunded_sender_units = total_escrow;
                    }
                    RulingOutcome::SplitEqual => {
                        pact_mut.state = PactState::Settled;
                        let half = total_escrow / 2;
                        pact_mut.settled_recipient_units = half;
                        pact_mut.refunded_sender_units = total_escrow - half;
                    }
                }
            }

            return Ok(Some(winning_outcome));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_escrow_settlement() {
        let mut engine = DisputeEngine::new();
        let pact_id = Uuid::new_v4();
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        engine.create_escrow_pact(
            pact_id,
            sender,
            recipient,
            5000,
            "commit_hash_123",
            10_000_000,
            vec![],
            0,
        );

        assert_eq!(engine.pacts.get(&pact_id).unwrap().state, PactState::Locked);
        engine.settle_normal(pact_id, sender).unwrap();

        let pact = engine.pacts.get(&pact_id).unwrap();
        assert_eq!(pact.state, PactState::Settled);
        assert_eq!(pact.settled_recipient_units, 5000);
        assert_eq!(pact.refunded_sender_units, 0);
    }

    #[test]
    fn test_timeout_slashing() {
        let mut engine = DisputeEngine::new();
        let pact_id = Uuid::new_v4();
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();

        engine.create_escrow_pact(
            pact_id,
            sender,
            recipient,
            3000,
            "commit_hash_456",
            5_000,
            vec![],
            0,
        );

        // Before timeout -> error
        let err = engine.execute_timeout_slash(pact_id, 4000).unwrap_err();
        assert_eq!(
            err,
            DisputeError::PactNotExpired {
                now_micros: 4000,
                timeout_micros: 5000
            }
        );

        // After timeout -> slashed
        engine.execute_timeout_slash(pact_id, 6000).unwrap();
        let pact = engine.pacts.get(&pact_id).unwrap();
        assert_eq!(pact.state, PactState::Slashed);
        assert_eq!(pact.refunded_sender_units, 3000);
    }

    #[test]
    fn test_dispute_arbitration_quorum() {
        let mut engine = DisputeEngine::new();
        let pact_id = Uuid::new_v4();
        let sender = Uuid::new_v4();
        let recipient = Uuid::new_v4();
        let arb1 = Uuid::new_v4();
        let arb2 = Uuid::new_v4();
        let arb3 = Uuid::new_v4();

        engine.create_escrow_pact(
            pact_id,
            sender,
            recipient,
            10_000,
            "commit_xyz",
            20_000,
            vec![arb1, arb2, arb3],
            2,
        );

        let ev = DisputeEvidence {
            evidence_id: Uuid::new_v4(),
            submitter_node_id: sender,
            violation_code: "NONCE_REPLAY".into(),
            payload_hash: "hash1".into(),
            signature: "sig1".into(),
        };

        engine.open_dispute(pact_id, sender, ev, 1000).unwrap();
        assert_eq!(
            engine.pacts.get(&pact_id).unwrap().state,
            PactState::Disputed
        );

        // Arb1 votes SlashRefundToSender
        let r1 = engine
            .submit_arbitration_vote(
                pact_id,
                arb1,
                RulingOutcome::SlashRefundToSender,
                "sig_arb1",
            )
            .unwrap();
        assert_eq!(r1, None);

        // Arb2 votes SlashRefundToSender -> Quorum reached (2-of-3)
        let r2 = engine
            .submit_arbitration_vote(
                pact_id,
                arb2,
                RulingOutcome::SlashRefundToSender,
                "sig_arb2",
            )
            .unwrap();
        assert_eq!(r2, Some(RulingOutcome::SlashRefundToSender));

        let pact = engine.pacts.get(&pact_id).unwrap();
        assert_eq!(pact.state, PactState::Slashed);
        assert_eq!(pact.refunded_sender_units, 10_000);
    }
}
