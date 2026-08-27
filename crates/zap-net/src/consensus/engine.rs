//! 2-Phase BFT Swarm Consensus State Machine implementation.

use ed25519_dalek::{SigningKey, VerifyingKey};
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};
use uuid::Uuid;

use super::{
    certificate::SwarmCommitCertificate,
    mod_types::ConsensusError,
    proposal::SwarmProposal,
    validator_set::ValidatorSet,
    vote::{SwarmVote, VoteKind},
};

pub trait SwarmConsensusEngine: Send + Sync {
    fn propose(
        &self,
        payload_digest: [u8; 32],
        state_merkle_root: [u8; 32],
    ) -> Result<SwarmProposal, ConsensusError>;
    fn handle_proposal(&self, proposal: SwarmProposal)
    -> Result<Option<SwarmVote>, ConsensusError>;
    fn handle_vote(
        &self,
        vote: SwarmVote,
    ) -> Result<Option<SwarmCommitCertificate>, ConsensusError>;
    fn advance_round(&self);
    fn reconfigure_epoch(&self, new_set: ValidatorSet);
}

#[derive(Debug, Clone, Default)]
struct RoundState {
    proposal: Option<SwarmProposal>,
    prevotes: HashMap<Uuid, SwarmVote>,
    precommits: HashMap<Uuid, SwarmVote>,
    polka_digest: Option<[u8; 32]>,
    committed_certificate: Option<SwarmCommitCertificate>,
}

pub struct BftConsensusEngine {
    self_node_id: Uuid,
    signing_key: SigningKey,
    epoch: Mutex<u64>,
    view: Mutex<u64>,
    round: Mutex<u64>,
    block_height: Mutex<u64>,
    validator_set: Mutex<ValidatorSet>,
    round_states: Mutex<HashMap<(u64, u64), RoundState>>,
    slashed_nodes: Mutex<HashSet<Uuid>>,
}

impl BftConsensusEngine {
    #[must_use]
    pub fn new(self_node_id: Uuid, signing_key: SigningKey, validator_set: ValidatorSet) -> Self {
        Self {
            self_node_id,
            signing_key,
            epoch: Mutex::new(validator_set.epoch),
            view: Mutex::new(0),
            round: Mutex::new(0),
            block_height: Mutex::new(1),
            validator_set: Mutex::new(validator_set),
            round_states: Mutex::new(HashMap::new()),
            slashed_nodes: Mutex::new(HashSet::new()),
        }
    }

    #[must_use]
    pub fn current_round(&self) -> u64 {
        *self.round.lock().unwrap()
    }

    #[must_use]
    pub fn current_epoch(&self) -> u64 {
        *self.epoch.lock().unwrap()
    }

    /// Block height that will be assigned to the next proposal.
    #[must_use]
    pub fn next_block_height(&self) -> u64 {
        *self.block_height.lock().unwrap()
    }

    #[must_use]
    pub fn is_slashed(&self, node_id: &Uuid) -> bool {
        self.slashed_nodes.lock().unwrap().contains(node_id)
    }

    pub fn slash_node(&self, offender: Uuid) {
        self.slashed_nodes.lock().unwrap().insert(offender);
    }
}

impl SwarmConsensusEngine for BftConsensusEngine {
    fn propose(
        &self,
        payload_digest: [u8; 32],
        state_merkle_root: [u8; 32],
    ) -> Result<SwarmProposal, ConsensusError> {
        let epoch = *self.epoch.lock().unwrap();
        let view = *self.view.lock().unwrap();
        let round = *self.round.lock().unwrap();
        let block_height = *self.block_height.lock().unwrap();

        let val_set = self.validator_set.lock().unwrap().clone();
        let expected_proposer = val_set.proposer_for_round(view, round);
        if expected_proposer.node_id != self.self_node_id {
            return Err(ConsensusError::UnauthorizedProposer {
                proposer: self.self_node_id,
                epoch,
                round,
            });
        }

        let now_micros = zap_core::now_micros().unwrap_or(0);
        let proposal = SwarmProposal::new_signed(
            epoch,
            view,
            round,
            block_height,
            self.self_node_id,
            payload_digest,
            state_merkle_root,
            None,
            now_micros,
            &self.signing_key,
        );

        let mut states = self.round_states.lock().unwrap();
        let round_state = states.entry((epoch, round)).or_default();
        round_state.proposal = Some(proposal.clone());

        Ok(proposal)
    }

    fn handle_proposal(
        &self,
        proposal: SwarmProposal,
    ) -> Result<Option<SwarmVote>, ConsensusError> {
        let current_epoch = *self.epoch.lock().unwrap();
        if proposal.epoch != current_epoch {
            return Err(ConsensusError::EpochMismatch {
                cert_epoch: proposal.epoch,
                set_epoch: current_epoch,
            });
        }
        let current_view = *self.view.lock().unwrap();
        let current_round = *self.round.lock().unwrap();
        if proposal.view != current_view || proposal.round != current_round {
            return Err(ConsensusError::RoundMismatch {
                message_view: proposal.view,
                message_round: proposal.round,
                current_view,
                current_round,
            });
        }

        if self.is_slashed(&proposal.proposer_node) {
            return Err(ConsensusError::InvalidProposalSignature(
                proposal.proposer_node,
            ));
        }

        let val_set = self.validator_set.lock().unwrap().clone();
        let expected_proposer = val_set.proposer_for_round(proposal.view, proposal.round);
        if expected_proposer.node_id != proposal.proposer_node {
            return Err(ConsensusError::UnauthorizedProposer {
                proposer: proposal.proposer_node,
                epoch: proposal.epoch,
                round: proposal.round,
            });
        }

        let vk = VerifyingKey::from_bytes(&expected_proposer.public_key)
            .map_err(|_| ConsensusError::InvalidValidatorKey(expected_proposer.node_id))?;
        if !proposal.verify_signature(&vk) {
            return Err(ConsensusError::InvalidProposalSignature(
                proposal.proposer_node,
            ));
        }

        let now_micros = zap_core::now_micros().unwrap_or(0);
        let prevote = SwarmVote::new_signed(
            proposal.epoch,
            proposal.view,
            proposal.round,
            VoteKind::Prevote,
            proposal.payload_digest,
            self.self_node_id,
            now_micros,
            &self.signing_key,
        );

        let mut states = self.round_states.lock().unwrap();
        let round_state = states.entry((proposal.epoch, proposal.round)).or_default();
        round_state.proposal = Some(proposal);
        round_state
            .prevotes
            .insert(self.self_node_id, prevote.clone());

        Ok(Some(prevote))
    }

    fn handle_vote(
        &self,
        vote: SwarmVote,
    ) -> Result<Option<SwarmCommitCertificate>, ConsensusError> {
        let current_epoch = *self.epoch.lock().unwrap();
        if vote.epoch != current_epoch {
            return Err(ConsensusError::EpochMismatch {
                cert_epoch: vote.epoch,
                set_epoch: current_epoch,
            });
        }
        let current_view = *self.view.lock().unwrap();
        let current_round = *self.round.lock().unwrap();
        if vote.view != current_view || vote.round != current_round {
            return Err(ConsensusError::RoundMismatch {
                message_view: vote.view,
                message_round: vote.round,
                current_view,
                current_round,
            });
        }
        if self.is_slashed(&vote.voter_node) {
            return Err(ConsensusError::InvalidVoteSignature(vote.voter_node));
        }

        let val_set = self.validator_set.lock().unwrap().clone();
        let val_entry = val_set
            .get_validator(&vote.voter_node)
            .ok_or(ConsensusError::InvalidValidatorKey(vote.voter_node))?;
        let vk = VerifyingKey::from_bytes(&val_entry.public_key)
            .map_err(|_| ConsensusError::InvalidValidatorKey(vote.voter_node))?;

        if !vote.verify_signature(&vk) {
            return Err(ConsensusError::InvalidVoteSignature(vote.voter_node));
        }

        let mut states = self.round_states.lock().unwrap();
        let round_state = states.entry((vote.epoch, vote.round)).or_default();

        // A certificate is emitted once per round. Re-emitting the same certificate
        // for delayed packets would cause duplicate application at the node layer.
        if round_state.committed_certificate.is_some() {
            return Ok(None);
        }

        match vote.vote_kind {
            VoteKind::Prevote => {
                // Check equivocation
                if let Some(existing) = round_state.prevotes.get(&vote.voter_node)
                    && existing.proposal_digest != vote.proposal_digest
                {
                    self.slash_node(vote.voter_node);
                    return Err(ConsensusError::EquivocationDetected {
                        offender: vote.voter_node,
                        epoch: vote.epoch,
                        round: vote.round,
                    });
                }
                round_state.prevotes.insert(vote.voter_node, vote.clone());

                // Check Polka (Prevote >= Threshold)
                let matching_prevotes = round_state
                    .prevotes
                    .values()
                    .filter(|v| v.proposal_digest == vote.proposal_digest)
                    .count();

                if matching_prevotes >= val_set.threshold as usize
                    && round_state.polka_digest.is_none()
                {
                    round_state.polka_digest = Some(vote.proposal_digest);
                    // Automatically generate local Precommit vote (canonical timestamp 0:
                    // certificates carry no timestamps, so batch verification requires
                    // every precommit to be signed over the canonical digest).
                    let precommit = SwarmVote::new_signed(
                        vote.epoch,
                        vote.view,
                        vote.round,
                        VoteKind::Precommit,
                        vote.proposal_digest,
                        self.self_node_id,
                        0,
                        &self.signing_key,
                    );
                    round_state.precommits.insert(self.self_node_id, precommit);
                }
            }
            VoteKind::Precommit => {
                // Check equivocation
                if let Some(existing) = round_state.precommits.get(&vote.voter_node)
                    && existing.proposal_digest != vote.proposal_digest
                {
                    self.slash_node(vote.voter_node);
                    return Err(ConsensusError::EquivocationDetected {
                        offender: vote.voter_node,
                        epoch: vote.epoch,
                        round: vote.round,
                    });
                }
                round_state.precommits.insert(vote.voter_node, vote.clone());

                // Check Commit (Precommit >= Threshold)
                let matching_precommits: Vec<SwarmVote> = round_state
                    .precommits
                    .values()
                    .filter(|v| v.proposal_digest == vote.proposal_digest)
                    .cloned()
                    .collect();

                if matching_precommits.len() >= val_set.threshold as usize
                    && round_state.committed_certificate.is_none()
                {
                    // Deterministically order signers by validator-set position so the
                    // certificate's signatures align with `resolve_bitmask_signers`
                    // (which enumerates the set in validator order). HashMap iteration
                    // order is random, so an unsorted pairing would verify each
                    // signature against the wrong signer's message.
                    let mut ordered: Vec<(usize, Uuid, [u8; 64])> = matching_precommits
                        .iter()
                        .map(|v| {
                            let idx = val_set
                                .validators
                                .iter()
                                .position(|e| e.node_id == v.voter_node)
                                .unwrap_or(usize::MAX);
                            (idx, v.voter_node, v.signature)
                        })
                        .collect();
                    ordered.sort_by_key(|(idx, _, _)| *idx);
                    let signers: Vec<Uuid> = ordered.iter().map(|(_, id, _)| *id).collect();
                    let signatures: Vec<[u8; 64]> = ordered.iter().map(|(_, _, s)| *s).collect();

                    let signer_bitmask = val_set.create_bitmask(&signers);
                    let block_height = *self.block_height.lock().unwrap();

                    let cert = SwarmCommitCertificate {
                        epoch: vote.epoch,
                        view: vote.view,
                        round: vote.round,
                        block_height,
                        proposal_digest: vote.proposal_digest,
                        threshold: val_set.threshold,
                        total_validators: val_set.validators.len() as u16,
                        signer_bitmask,
                        signatures,
                    };

                    round_state.committed_certificate = Some(cert.clone());
                    drop(states);

                    // Seal the committed height and close the round locally. This
                    // prevents a later proposal from reusing the same block height
                    // or delayed votes from reopening a finalized round.
                    {
                        let mut height = self.block_height.lock().unwrap();
                        if *height == block_height {
                            *height += 1;
                        }
                    }
                    {
                        let mut round = self.round.lock().unwrap();
                        if *round == vote.round {
                            *round += 1;
                        }
                    }
                    return Ok(Some(cert));
                }
            }
        }

        Ok(None)
    }

    fn advance_round(&self) {
        let mut r = self.round.lock().unwrap();
        *r += 1;
    }

    fn reconfigure_epoch(&self, new_set: ValidatorSet) {
        let mut epoch = self.epoch.lock().unwrap();
        *epoch = new_set.epoch;
        let mut val_set = self.validator_set.lock().unwrap();
        *val_set = new_set;
        let mut round = self.round.lock().unwrap();
        *round = 0;
    }
}
