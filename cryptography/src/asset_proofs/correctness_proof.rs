//! The proof of correct encryption of the given value.
//! For more details see section 5.2 of the whitepaper.

use crate::{
    asset_proofs::{
        encryption_proofs::{
            AssetProofProver, AssetProofProverAwaitingChallenge, AssetProofVerifier, ZKPChallenge,
        },
        transcript::{TranscriptProtocol, UpdateTranscript},
        CipherText, CommitmentWitness, ElgamalPublicKey,
    },
    errors::{ErrorKind, Fallible},
};
use bulletproofs::PedersenGens;
use curve25519_dalek::{
    constants::RISTRETTO_BASEPOINT_POINT, ristretto::RistrettoPoint, scalar::Scalar,
};
use merlin::{Transcript, TranscriptRng};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use std::convert::From;
use zeroize::Zeroize;

/// The domain label for the correctness proof.
pub const CORRECTNESS_PROOF_FINAL_RESPONSE_LABEL: &[u8] = b"PolymathCorrectnessFinalResponse";
/// The domain label for the challenge.
pub const CORRECTNESS_PROOF_CHALLENGE_LABEL: &[u8] = b"PolymathCorrectnessChallenge";

// ------------------------------------------------------------------------
// Proof of Correct Encryption of the Given Value
// ------------------------------------------------------------------------

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug, Default)]
pub struct CorrectnessFinalResponse(Scalar);

impl From<Scalar> for CorrectnessFinalResponse {
    fn from(response: Scalar) -> Self {
        CorrectnessFinalResponse(response)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone, Debug)]
pub struct CorrectnessInitialMessage {
    a: RistrettoPoint,
    b: RistrettoPoint,
}

/// A default implementation used for testing.
impl Default for CorrectnessInitialMessage {
    fn default() -> Self {
        CorrectnessInitialMessage {
            a: RISTRETTO_BASEPOINT_POINT,
            b: RISTRETTO_BASEPOINT_POINT,
        }
    }
}

impl UpdateTranscript for CorrectnessInitialMessage {
    fn update_transcript(&self, transcript: &mut Transcript) -> Fallible<()> {
        transcript.append_domain_separator(CORRECTNESS_PROOF_CHALLENGE_LABEL);
        transcript.append_validated_point(b"A", &self.a.compress())?;
        transcript.append_validated_point(b"B", &self.b.compress())?;
        Ok(())
    }
}

pub struct CorrectnessProverAwaitingChallenge<'a> {
    /// The public key used for the elgamal encryption.
    pub_key: ElgamalPublicKey,

    /// The secret commitment witness.
    w: CommitmentWitness,

    /// Pedersen Generators
    pc_gens: &'a PedersenGens,
}

impl<'a> CorrectnessProverAwaitingChallenge<'a> {
    pub fn new(pub_key: ElgamalPublicKey, w: CommitmentWitness, pc_gens: &'a PedersenGens) -> Self {
        CorrectnessProverAwaitingChallenge {
            pub_key,
            w,
            pc_gens,
        }
    }
}

#[derive(Zeroize)]
#[zeroize(drop)]
pub struct CorrectnessProver {
    /// The secret commitment witness.
    w: CommitmentWitness,

    /// The randomness generate in the first round.
    u: Scalar,
}

impl<'a> AssetProofProverAwaitingChallenge for CorrectnessProverAwaitingChallenge<'a> {
    type ZKInitialMessage = CorrectnessInitialMessage;
    type ZKFinalResponse = CorrectnessFinalResponse;
    type ZKProver = CorrectnessProver;

    fn create_transcript_rng<T: RngCore + CryptoRng>(
        &self,
        rng: &mut T,
        transcript: &Transcript,
    ) -> TranscriptRng {
        transcript.create_transcript_rng_from_witness(rng, &self.w)
    }

    fn generate_initial_message(
        &self,
        rng: &mut TranscriptRng,
    ) -> (Self::ZKProver, Self::ZKInitialMessage) {
        let rand_commitment = Scalar::random(rng);

        (
            CorrectnessProver {
                w: self.w.clone(),
                u: rand_commitment,
            },
            CorrectnessInitialMessage {
                a: rand_commitment * self.pub_key.pub_key,
                b: rand_commitment * self.pc_gens.B_blinding,
            },
        )
    }
}

impl AssetProofProver<CorrectnessFinalResponse> for CorrectnessProver {
    fn apply_challenge(&self, c: &ZKPChallenge) -> CorrectnessFinalResponse {
        CorrectnessFinalResponse(self.u + c.x() * self.w.blinding())
    }
}

pub struct CorrectnessVerifier<'a> {
    /// The encrypted value (aka the plain text).
    value: u32,

    /// The public key to which the `value` is encrypted.
    pub_key: ElgamalPublicKey,

    /// The encryption cipher text.
    cipher: CipherText,

    /// The Generator Points
    pc_gens: &'a PedersenGens,
}

impl<'a> CorrectnessVerifier<'a> {
    pub fn new(
        value: u32,
        pub_key: ElgamalPublicKey,
        cipher: CipherText,
        pc_gens: &'a PedersenGens,
    ) -> Self {
        CorrectnessVerifier {
            value,
            pub_key,
            cipher,
            pc_gens,
        }
    }
}

impl<'a> AssetProofVerifier for CorrectnessVerifier<'a> {
    type ZKInitialMessage = CorrectnessInitialMessage;
    type ZKFinalResponse = CorrectnessFinalResponse;

    fn verify(
        &self,
        challenge: &ZKPChallenge,
        initial_message: &Self::ZKInitialMessage,
        z: &Self::ZKFinalResponse,
    ) -> Fallible<()> {
        let generators = self.pc_gens;
        let y_prime = self.cipher.y - (Scalar::from(self.value) * generators.B);

        ensure!(
            z.0 * self.pub_key.pub_key == initial_message.a + challenge.x() * self.cipher.x,
            ErrorKind::CorrectnessFinalResponseVerificationError { check: 1 }
        );
        ensure!(
            z.0 * generators.B_blinding == initial_message.b + challenge.x() * y_prime,
            ErrorKind::CorrectnessFinalResponseVerificationError { check: 2 }
        );
        Ok(())
    }
}

// ------------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    extern crate wasm_bindgen_test;
    use super::*;
    use crate::asset_proofs::*;
    use bincode::{deserialize, serialize};
    use rand::{rngs::StdRng, SeedableRng};
    use std::convert::TryFrom;
    use wasm_bindgen_test::*;

    const SEED_1: [u8; 32] = [17u8; 32];

    #[test]
    #[wasm_bindgen_test]
    fn test_correctness_proof() {
        let gens = PedersenGens::default();
        let mut rng = StdRng::from_seed(SEED_1);
        let secret_value = 13u32;
        let rand_blind = Scalar::random(&mut rng);

        let w = CommitmentWitness::try_from((secret_value, rand_blind)).unwrap();
        let elg_secret = ElgamalSecretKey::new(Scalar::random(&mut rng));
        let elg_pub = elg_secret.get_public_key();
        let cipher = elg_pub.encrypt(&w);

        let prover = CorrectnessProverAwaitingChallenge::new(elg_pub, w, &gens);
        let verifier = CorrectnessVerifier::new(secret_value, elg_pub, cipher, &gens);
        let mut transcript = Transcript::new(CORRECTNESS_PROOF_FINAL_RESPONSE_LABEL);

        // Positive tests
        let mut transcript_rng = prover.create_transcript_rng(&mut rng, &transcript);
        let (prover, initial_message) = prover.generate_initial_message(&mut transcript_rng);
        initial_message.update_transcript(&mut transcript).unwrap();
        let challenge = transcript
            .scalar_challenge(CORRECTNESS_PROOF_CHALLENGE_LABEL)
            .unwrap();
        let final_response = prover.apply_challenge(&challenge);

        let result = verifier.verify(&challenge, &initial_message, &final_response);
        assert!(result.is_ok());

        // Negative tests
        let bad_initial_message = CorrectnessInitialMessage::default();
        let result = verifier.verify(&challenge, &bad_initial_message, &final_response);
        assert_err!(
            result,
            ErrorKind::CorrectnessFinalResponseVerificationError { check: 1 }
        );

        let bad_final_response = CorrectnessFinalResponse(Scalar::default());
        let result = verifier.verify(&challenge, &initial_message, &bad_final_response);
        assert_err!(
            result,
            ErrorKind::CorrectnessFinalResponseVerificationError { check: 1 }
        );
    }

    #[test]
    #[wasm_bindgen_test]
    fn serialize_deserialize_proof() {
        let mut rng = StdRng::from_seed(SEED_1);
        let secret_value = 42u32;
        let secret_key = ElgamalSecretKey::new(Scalar::random(&mut rng));
        let pub_key = secret_key.get_public_key();
        let rand_blind = Scalar::random(&mut rng);
        let w = CommitmentWitness::try_from((secret_value, rand_blind)).unwrap();
        let gens = PedersenGens::default();
        let prover = CorrectnessProverAwaitingChallenge::new(pub_key, w, &gens);
        let (initial_message, final_response) = encryption_proofs::single_property_prover::<
            StdRng,
            CorrectnessProverAwaitingChallenge,
        >(prover, &mut rng)
        .unwrap();

        let initial_message_bytes: Vec<u8> = serialize(&initial_message).unwrap();
        let final_response_bytes: Vec<u8> = serialize(&final_response).unwrap();
        let recovered_initial_message: CorrectnessInitialMessage =
            deserialize(&initial_message_bytes).unwrap();
        let recovered_final_response: CorrectnessFinalResponse =
            deserialize(&final_response_bytes).unwrap();
        assert_eq!(recovered_initial_message, initial_message);
        assert_eq!(recovered_final_response, final_response);
    }
}