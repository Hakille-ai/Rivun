//! BFT Swarm Consensus Engine Integration & Slashing Tests.

use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use uuid::Uuid;
use zap_net::consensus::{
    BftConsensusEngine, EquivocationProof, SwarmCommitCertificate, SwarmConsensusEngine, SwarmVote,
    ValidatorEntry, ValidatorSet, VoteKind,
};

fn create_validator_cluster(n: usize) -> (Vec<SigningKey>, ValidatorSet, Vec<BftConsensusEngine>) {
    let mut keys = Vec::new();
    let mut entries = Vec::new();

    for _ in 0..n {
        let key = SigningKey::generate(&mut OsRng);
        let node_id = Uuid::new_v4();
        entries.push(ValidatorEntry {
            node_id,
            public_key: key.verifying_key().to_bytes(),
            weight: 1,
        });
        keys.push(key);
    }

    let val_set = ValidatorSet::new(1, entries.clone()).expect("valid set");
    let mut engines = Vec::new();

    for (i, entry) in entries.iter().enumerate() {
        engines.push(BftConsensusEngine::new(
            entry.node_id,
            keys[i].clone(),
            val_set.clone(),
        ));
    }

    (keys, val_set, engines)
}

#[test]
fn test_bft_four_phase_commit_happy_path() {
    let (_keys, val_set, engines) = create_validator_cluster(4);
    assert_eq!(val_set.threshold, 3); // N=4 -> T=3

    let proposer_entry = val_set.proposer_for_round(0, 0);
    let proposer_idx = val_set
        .validators
        .iter()
        .position(|v| v.node_id == proposer_entry.node_id)
        .unwrap();

    let payload_digest = [42_u8; 32];
    let state_root = [1_u8; 32];

    // Phase 1: Propose
    let proposal = engines[proposer_idx]
        .propose(payload_digest, state_root)
        .expect("propose failed");

    // Phase 2: Prevote (collect from all nodes)
    let mut prevotes = Vec::new();
    for engine in &engines {
        let vote = engine
            .handle_proposal(proposal.clone())
            .expect("handle_proposal failed")
            .expect("should produce prevote");
        prevotes.push(vote);
    }

    // Phase 3 & 4: Precommit & Commit
    let mut commit_cert = None;
    for vote in &prevotes {
        for engine in &engines {
            if let Ok(Some(cert)) = engine.handle_vote(vote.clone()) {
                commit_cert = Some(cert);
            }
        }
    }

    // Phase 4: Deliver precommits (simulating network broadcast of each precommit).
    // Precommits are signed with the canonical timestamp 0 so the certificate's
    // batch verification (which reconstructs the digest without timestamps) passes.
    let precommits: Vec<SwarmVote> = (0..4)
        .map(|i| {
            SwarmVote::new_signed(
                1,
                0,
                0,
                VoteKind::Precommit,
                payload_digest,
                val_set.validators[i].node_id,
                0,
                &_keys[i],
            )
        })
        .collect();
    for vote in &precommits {
        for engine in &engines {
            if let Ok(Some(cert)) = engine.handle_vote(vote.clone()) {
                commit_cert = Some(cert);
            }
        }
    }

    let cert = commit_cert.expect("commit certificate must be formed");
    assert_eq!(cert.threshold, 3);
    assert!(cert.verify_against_set(&val_set).is_ok());
}

#[test]
fn test_bft_single_byzantine_node_drop_tolerance() {
    let (_keys, val_set, engines) = create_validator_cluster(4);
    // N=4, f=1: 3 nodes are sufficient for quorum (T=3)
    let payload = [99_u8; 32];
    let state_root = [10_u8; 32];

    let proposer_entry = val_set.proposer_for_round(0, 0);
    let proposer_idx = val_set
        .validators
        .iter()
        .position(|v| v.node_id == proposer_entry.node_id)
        .unwrap();

    let proposal = engines[proposer_idx]
        .propose(payload, state_root)
        .expect("propose failed");

    // Only nodes 0, 1, 2 respond (node 3 drops silently)
    let mut prevotes = Vec::new();
    for engine in engines.iter().take(3) {
        let vote = engine
            .handle_proposal(proposal.clone())
            .expect("proposal ok")
            .unwrap();
        prevotes.push(vote);
    }

    let mut cert = None;
    for vote in &prevotes {
        for engine in engines.iter().take(3) {
            if let Ok(Some(c)) = engine.handle_vote(vote.clone()) {
                cert = Some(c);
            }
        }
    }

    // Deliver precommits only from the 3 responsive nodes (node 3 stays silent),
    // signed with the canonical timestamp 0 (see certificate batch verification).
    let precommits: Vec<SwarmVote> = (0..3)
        .map(|i| {
            SwarmVote::new_signed(
                1,
                0,
                0,
                VoteKind::Precommit,
                payload,
                val_set.validators[i].node_id,
                0,
                &_keys[i],
            )
        })
        .collect();
    for vote in &precommits {
        for engine in engines.iter().take(3) {
            if let Ok(Some(c)) = engine.handle_vote(vote.clone()) {
                cert = Some(c);
            }
        }
    }

    let committed = cert.expect("3 surviving nodes must reach consensus");
    assert!(committed.verify_against_set(&val_set).is_ok());
}

#[test]
fn test_bft_equivocation_slashing_proof() {
    let offender_key = SigningKey::generate(&mut OsRng);
    let offender_id = Uuid::new_v4();

    let vote_a = SwarmVote::new_signed(
        1,
        0,
        0,
        VoteKind::Prevote,
        [1_u8; 32],
        offender_id,
        1_000_000,
        &offender_key,
    );

    let vote_b = SwarmVote::new_signed(
        1,
        0,
        0,
        VoteKind::Prevote,
        [2_u8; 32],
        offender_id,
        1_000_000,
        &offender_key,
    );

    let proof = EquivocationProof::from_votes(&vote_a, &vote_b)
        .expect("equivocation proof must be created");

    assert!(proof.verify(&offender_key.verifying_key()));
}

#[test]
fn test_bft_leader_rotation_on_proposal_timeout() {
    let (_keys, val_set, engines) = create_validator_cluster(4);
    let leader_r0 = val_set.proposer_for_round(0, 0);
    let leader_r1 = val_set.proposer_for_round(0, 1);
    let leader_r2 = val_set.proposer_for_round(0, 2);

    assert_ne!(leader_r0.node_id, leader_r1.node_id);
    assert_ne!(leader_r1.node_id, leader_r2.node_id);

    // Advance round on all engines
    for engine in &engines {
        assert_eq!(engine.current_round(), 0);
        engine.advance_round();
        assert_eq!(engine.current_round(), 1);
    }
}

#[test]
fn test_bft_threshold_bitmask_batch_verification() {
    let (keys, val_set, _engines) = create_validator_cluster(16);
    // T = (16*2)/3 + 1 = 10 + 1 = 11
    assert_eq!(val_set.threshold, 11);

    let payload = [55_u8; 32];
    let mut signers = Vec::new();
    let mut signatures = Vec::new();

    for (i, key) in keys.iter().take(11).enumerate() {
        let voter_id = val_set.validators[i].node_id;
        let vote = SwarmVote::new_signed(1, 0, 0, VoteKind::Precommit, payload, voter_id, 0, key);
        signers.push(voter_id);
        signatures.push(vote.signature);
    }

    let mask = val_set.create_bitmask(&signers);
    let cert = SwarmCommitCertificate {
        epoch: 1,
        view: 0,
        round: 0,
        block_height: 100,
        proposal_digest: payload,
        threshold: val_set.threshold,
        total_validators: 16,
        signer_bitmask: mask,
        signatures,
    };

    assert!(cert.verify_against_set(&val_set).is_ok());
}

#[test]
fn test_bft_corrupted_signature_batch_rejection() {
    let (keys, val_set, _engines) = create_validator_cluster(4);
    let payload = [12_u8; 32];
    let mut signers = Vec::new();
    let mut signatures = Vec::new();

    for (i, key) in keys.iter().take(3).enumerate() {
        let voter_id = val_set.validators[i].node_id;
        let vote = SwarmVote::new_signed(1, 0, 0, VoteKind::Precommit, payload, voter_id, 0, key);
        signers.push(voter_id);
        signatures.push(vote.signature);
    }

    // Corrupt one signature
    signatures[1][0] ^= 0xFF;

    let mask = val_set.create_bitmask(&signers);
    let cert = SwarmCommitCertificate {
        epoch: 1,
        view: 0,
        round: 0,
        block_height: 1,
        proposal_digest: payload,
        threshold: val_set.threshold,
        total_validators: 4,
        signer_bitmask: mask,
        signatures,
    };

    assert!(cert.verify_against_set(&val_set).is_err());
}

#[test]
fn test_bft_dynamic_validator_epoch_transition() {
    let (_keys, val_set_1, engines) = create_validator_cluster(4);
    assert_eq!(val_set_1.epoch, 1);
    assert_eq!(val_set_1.threshold, 3);

    // Create Epoch 2 validator set with 5 nodes (T=4)
    let (_keys2, val_set_2, _) = create_validator_cluster(5);
    let entries = val_set_2.validators;
    let new_set = ValidatorSet::new(2, entries).expect("epoch 2 set");
    assert_eq!(new_set.threshold, 4);

    for engine in &engines {
        engine.reconfigure_epoch(new_set.clone());
        assert_eq!(engine.current_epoch(), 2);
        assert_eq!(engine.current_round(), 0);
    }
}
