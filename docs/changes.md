# Rapport des changements détectés

Date : 2026-08-16

Ce document a été généré automatiquement après une analyse Git locale.

## Aperçu

- Plusieurs fichiers suivis (tracked) ont été modifiés, incluant `Cargo.toml`, `Cargo.lock` et des sources Rust dans plusieurs crates.
- Un grand nombre de fichiers non suivis (untracked) ont été détectés — probablement de nouvelles sources, tests, ou fichiers temporaires.
- Le branchement local `main` pointe vers `origin/main` (pas de commits locaux non poussés détectés), ce qui indique que les modifications sont principalement non committées dans l'arbre de travail.

## Fichiers modifiés (tracked)

- Cargo.lock
- Cargo.toml
- crates/zap-agent/Cargo.toml
- crates/zap-agent/src/lib.rs
- crates/zap-cli/Cargo.toml
- crates/zap-cli/src/main.rs
- crates/zap-crypto/src/lib.rs
- crates/zap-driver-sdk/Cargo.toml
- crates/zap-driver-sdk/src/lib.rs
- crates/zap-journal/src/lib.rs
- crates/zap-ledger/Cargo.toml
- crates/zap-ledger/src/lib.rs
- crates/zap-net/Cargo.toml
- crates/zap-net/src/lib.rs
- crates/zap-node/Cargo.toml
- crates/zap-node/src/lib.rs
- crates/zap-pact/src/lib.rs
- crates/zap-runtime/Cargo.toml
- crates/zap-runtime/src/lib.rs
- crates/zap-store/Cargo.toml
- crates/zap-store/src/lib.rs
- sdks/go/protocol_test.go
- sdks/go/zapstore.go
- sdks/rust/Cargo.lock
- sdks/rust/Cargo.toml
- sdks/rust/src/lib.rs

## Fichiers non suivis (untracked)

(Extrait)

- .agents/
- PROJECT.md
- TEST_INFRA.md
- TEST_READY.md
- crates/zap-agent/src/provenance.rs
- crates/zap-agent/src/swarm.rs
- crates/zap-cli/tests/gateway_cli_tests.rs
- crates/zap-cli/tests/pack_cli_tests.rs
- crates/zap-driver-sdk/src/async_driver.rs
- crates/zap-driver-sdk/src/buffer.rs
- crates/zap-driver-sdk/src/error.rs
- crates/zap-driver-sdk/src/ipc.rs
- crates/zap-gateway/
- crates/zap-journal/tests/
- crates/zap-ledger/src/batch.rs
- crates/zap-ledger/src/mmr.rs
- crates/zap-ledger/src/zk.rs
- crates/zap-ledger/tests/
- crates/zap-net/src/consensus/
- crates/zap-net/src/durable_replay.rs
- crates/zap-net/src/gossip/
- crates/zap-net/src/mesh/
- crates/zap-net/src/serde_helpers.rs
- crates/zap-net/tests/
- crates/zap-node/src/actors/
- crates/zap-node/src/config.rs
- crates/zap-node/src/durable_replay.rs
- crates/zap-node/tests/
- crates/zap-pack/
- crates/zap-pact/src/dispute.rs
- crates/zap-runtime/src/async_engine.rs
- crates/zap-runtime/src/ipc.rs
- crates/zap-runtime/src/pipeline.rs
- crates/zap-runtime/src/streaming.rs
- crates/zap-store/src/audit.rs
- crates/zap-store/src/bundle.rs
- crates/zap-store/src/resolver.rs
- crates/zap-store/src/validator.rs
- crates/zap-store/tests/
- crates/zap-telemetry/
- snapshot.tar.gz
- tests/

## Recommandations immédiates

1. Passez en revue les fichiers suivis modifiés (`git diff`) et validez uniquement les changements intentionnels.
2. Pour les fichiers non suivis :
   - Ajouter au contrôle de version s'ils sont nécessaires, ou
   - Ajouter à `.gitignore` s'ils sont temporaires (ex. `snapshot.tar.gz`), ou
   - Supprimer s'ils sont des artefacts non voulus.
3. Exécuter la suite de tests après commit : `cargo test --workspace --all-targets`.
4. Si tout est OK, commit puis push :

```bash
# Ajouter tous les changements intentionnels
git add -A
# Commit avec message clair
git commit -m "docs: update README and add automatic changes summary"
# Pousser
git push
```

## Notes supplémentaires

- Si vous voulez, je peux :
  - Créer automatiquement un commit contenant uniquement les changements de docs/README.
  - Ouvrir une PR via la remote (si vous me dites la branche cible).
  - Lancer les tests et vous transmettre la sortie.


-- rapport généré automatiquement
