//! A simple commandline application to demonstrate a claim prover's (AKA an investor)
//! steps to create proofs for their claims.
//! Use `polymath-scp --help` to see the usage.
//!

use cli_common::{
    make_message, InvestorDID, Proof, ScopeDID, UniqueID, INVESTORDID_LEN, SCOPEDID_LEN,
    UNIQUEID_LEN,
};
use confidential_identity::{
    build_scope_claim_proof_data, compute_cdd_id, compute_scope_id, mocked, CddClaimData,
    ProofKeyPair, ScopeClaimData,
};
use curve25519_dalek::ristretto::RistrettoPoint;
use hex;
use rand::{rngs::StdRng, SeedableRng};
use rand_core::{CryptoRng, RngCore};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

#[derive(Debug, Serialize, Deserialize)]
pub struct CddId {
    pub cdd_id: RistrettoPoint,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawCddClaimData {
    pub investor_did: InvestorDID,
    pub investor_unique_id: UniqueID,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawScopeClaimData {
    pub scope_did: ScopeDID,
    pub investor_unique_id: UniqueID,
}

/// polymath-scp -- a simple claim prover.
///
/// The polymath-scp/create-claim-proof utility which creates a proof for a scope-claim.
/// This CLI will generate random content for `cdd-claim` and `scope-claim` parameters if you
/// pass `-r` to it. Alternatively, you can provide the `cdd-claim` file that you have received
/// from CDD provider and generate your own `scope-claim` file.
///
/// Terminology:{n}
/// - cdd-claim privately links an investor unique identity to their on-chain identity (DID).
///             This is given to the users by a trusted CDD provider.{n}
/// - scope-claim is generated by the investor and links their DID to a scope
///               (for example, to an asset).{n}
/// - message, a message that indicates a claim that an investor intends to link their DID to
///            a scope.{n}
/// - proof, is the output of this CLI.
///          It contains a secure and private proof about the claim of the investor.
#[derive(Clone, Debug, StructOpt)]
pub struct CreateClaimProofInfo {
    /// Generate and use a random claim.
    #[structopt(short, long)]
    rand: bool,

    /// Get the Json formatted claim from file.
    /// If this option is provided along with `rand`,
    /// it will save the randomly generated claim to file.
    #[structopt(short, long, parse(from_os_str))]
    cdd_claim: Option<std::path::PathBuf>,

    /// The investor provided input which claims the investor DID holds an asset in a
    /// certain scope.
    #[structopt(short, long, parse(from_os_str))]
    scope_claim: Option<std::path::PathBuf>,

    /// Write the proof to file in Json format.
    #[structopt(short, long, parse(from_os_str))]
    proof: Option<std::path::PathBuf>,

    /// Be verbose.
    #[structopt(short, long)]
    verbose: bool,
}

/// polymath-scp -- a simple claim prover.
///
/// The polymath-scp/create-cdd-id utility which creates a CDD Id for a CDD Claim.
/// This CLI will generate random content for `cdd-claim` parameter if you pass `-r` to it.
/// Alternatively, you can provide the `cdd-claim` file that you have created as a CDD provider.
///
/// Terminology:{n}
/// - cdd-claim privately links an investor unique identity to their on-chain identity (DID).
///             This is created for the users by a trusted CDD provider.{n}
/// - cdd-id is an on-chain ID that is created by the CDD provider and links the investor's
///          DID to their unique identity.
#[derive(Clone, Debug, StructOpt)]
pub struct CreateCDDIdInfo {
    /// Generate and use a random CDD claim.
    #[structopt(short, long)]
    rand: bool,

    /// Get the Json formatted claim from file.
    /// If this option is provided along with `rand`,
    /// it will save the randomly generated claim to file.
    #[structopt(short, long, parse(from_os_str))]
    cdd_claim: Option<std::path::PathBuf>,

    /// Write the CDD Id to file in Json format.
    #[structopt(long, parse(from_os_str))]
    cdd_id: Option<std::path::PathBuf>,

    /// Be verbose.
    #[structopt(short, long)]
    verbose: bool,
}

/// The polymath-scp/create-cdd-id utility which creates an Identity with a mocked CDD Id.
#[derive(Clone, Debug, StructOpt)]
pub struct CreateMockedInvestorUidInfo {
    /// Input DID in hex, i.e "0x0600000000000000000000000000000000000000000000000000000000000000"
    #[structopt(short, long)]
    did: String,

    /// Output as standard string format, i.e "cae66941-d9ef-4d40-8e4d-88758ea67670"
    #[structopt(short, long)]
    formatted: bool,
}

#[derive(Clone, Debug, StructOpt)]
pub enum CLI {
    /// Create the CDD Id.
    CreateCDDId(CreateCDDIdInfo),

    /// Create a Claim proof.
    CreateClaimProof(CreateClaimProofInfo),

    /// Create Mocked CDD Id.
    CreateMockedInvestorUid(CreateMockedInvestorUidInfo),
}

/// Generate a random `InvestorDID` for experiments.
fn random_investor_did<R: RngCore + CryptoRng>(rng: &mut R) -> InvestorDID {
    let mut investor_did = [0u8; INVESTORDID_LEN];
    rng.fill_bytes(&mut investor_did);
    investor_did
}

/// Generate a random `ScopeDID` for experiments.
fn random_scope_did<R: RngCore + CryptoRng>(rng: &mut R) -> ScopeDID {
    let mut scope_did = [0u8; SCOPEDID_LEN];
    rng.fill_bytes(&mut scope_did);
    scope_did
}

/// Generate a random `UniqueID` for experiments.
fn random_unique_id<R: RngCore + CryptoRng>(rng: &mut R) -> UniqueID {
    let mut unique_id = [0u8; UNIQUEID_LEN];
    rng.fill_bytes(&mut unique_id);
    unique_id
}

fn process_create_cdd_id(cfg: CreateCDDIdInfo) {
    let raw_cdd_data = if cfg.rand {
        let mut rng = StdRng::from_seed([42u8; 32]);
        let rand_investor_did = random_investor_did(&mut rng);
        let rand_unique_id = random_unique_id(&mut rng);
        let raw_cdd_data = RawCddClaimData {
            investor_did: rand_investor_did,
            investor_unique_id: rand_unique_id,
        };

        // If user provided the `claim` option, save this to file.
        if let Some(c) = cfg.cdd_claim {
            std::fs::write(
                c,
                serde_json::to_string(&raw_cdd_data)
                    .unwrap_or_else(|error| panic!("Failed to serialize the cdd claim: {}", error)),
            )
            .expect("Failed to write the cdd claim to file.");
            if cfg.verbose {
                println!("Successfully wrote the cdd claim to file.");
            }
        }

        raw_cdd_data
    } else {
        let file_cdd_claim = match cfg.cdd_claim {
            Some(c) => {
                let json_file_content =
                    std::fs::read_to_string(&c).expect("Failed to read the cdd claim from file.");
                serde_json::from_str(&json_file_content).unwrap_or_else(|error| {
                    panic!("Failed to deserialize the cdd claim: {}", error)
                })
            }
            None => panic!("You must either pass in a claim file or generate it randomly."),
        };

        file_cdd_claim
    };

    let cdd_claim = CddClaimData::new(&raw_cdd_data.investor_did, &raw_cdd_data.investor_unique_id);

    if cfg.verbose {
        println!(
            "CDD Claim: {:?}",
            serde_json::to_string(&cdd_claim).unwrap()
        );
    }

    let cdd_id = compute_cdd_id(&cdd_claim);

    // => CDD provider includes the CDD Id in their claim and submits it to the PolyMesh.
    let packaged_cdd_id = CddId { cdd_id: cdd_id };
    let cdd_id_str = serde_json::to_string(&packaged_cdd_id)
        .unwrap_or_else(|error| panic!("Failed to serialize the CDD Id: {}", error));

    if cfg.verbose {
        println!("CDD Id Package: {:?}", cdd_id_str);
    }

    if let Some(p) = cfg.cdd_id {
        std::fs::write(p, cdd_id_str.as_bytes()).expect("Failed to write the CDD Id to file.");
        println!("Successfully wrote the CDD Id.");
    }
}

fn process_create_claim_proof(cfg: CreateClaimProofInfo) {
    let (raw_cdd_claim, raw_scope_claim) = if cfg.rand {
        let mut rng = StdRng::from_seed([42u8; 32]);
        // let (rand_cdd_claim, rand_scope_claim) = random_claim(&mut rng);
        let rand_investor_did = random_investor_did(&mut rng);
        let rand_unique_id = random_unique_id(&mut rng);
        let raw_cdd_data = RawCddClaimData {
            investor_did: rand_investor_did,
            investor_unique_id: rand_unique_id.clone(),
        };

        let rand_scope_did = random_scope_did(&mut rng);
        let raw_scope_data = RawScopeClaimData {
            scope_did: rand_scope_did,
            investor_unique_id: rand_unique_id,
        };

        // If user provided the `claim` option, save this to file.
        if let Some(c) = cfg.cdd_claim {
            std::fs::write(
                c,
                serde_json::to_string(&raw_cdd_data)
                    .unwrap_or_else(|error| panic!("Failed to serialize the cdd claim: {}", error)),
            )
            .expect("Failed to write the cdd claim to file.");
            if cfg.verbose {
                println!("Successfully wrote the cdd claim to file.");
            }
        }

        if let Some(c) = cfg.scope_claim {
            std::fs::write(
                c,
                serde_json::to_string(&raw_scope_data).unwrap_or_else(|error| {
                    panic!("Failed to serialize the scope claim: {}", error)
                }),
            )
            .expect("Failed to write the scope claim to file.");
            if cfg.verbose {
                println!("Successfully wrote the scope claim to file.");
            }
        }

        (raw_cdd_data, raw_scope_data)
    } else {
        let file_cdd_claim = match cfg.cdd_claim {
            Some(c) => {
                let json_file_content =
                    std::fs::read_to_string(&c).expect("Failed to read the cdd claim from file.");
                serde_json::from_str(&json_file_content).unwrap_or_else(|error| {
                    panic!("Failed to deserialize the cdd claim: {}", error)
                })
            }
            None => panic!("You must either pass in a claim file or generate it randomly."),
        };
        let file_scope_claim = match cfg.scope_claim {
            Some(c) => {
                let json_file_content =
                    std::fs::read_to_string(&c).expect("Failed to read the scope claim from file.");
                serde_json::from_str(&json_file_content).unwrap_or_else(|error| {
                    panic!("Failed to deserialize the scope claim: {}", error)
                })
            }
            None => panic!("You must either pass in a claim file or generate it randomly."),
        };
        (file_cdd_claim, file_scope_claim)
    };

    let message = make_message(&raw_cdd_claim.investor_did, &raw_scope_claim.scope_did);

    if cfg.verbose {
        println!(
            "CDD Claim: {:?}",
            serde_json::to_string(&raw_cdd_claim).unwrap()
        );
        println!(
            "Scope Claim: {:?}",
            serde_json::to_string(&raw_scope_claim).unwrap()
        );
        println!("Message: {:?}", message);
    }

    let cdd_claim = CddClaimData::new(
        &raw_cdd_claim.investor_did,
        &raw_cdd_claim.investor_unique_id,
    );
    let scope_claim = ScopeClaimData::new(
        &raw_scope_claim.scope_did,
        &raw_scope_claim.investor_unique_id,
    );
    let scope_claim_proof_data = build_scope_claim_proof_data(&cdd_claim, &scope_claim);

    let pair = ProofKeyPair::from(scope_claim_proof_data);
    let proof = pair.generate_id_match_proof(&message).to_bytes().to_vec();

    let cdd_id = compute_cdd_id(&cdd_claim);
    let scope_id = compute_scope_id(&scope_claim);

    // => Investor makes {cdd_id, investor_did, scope_id, scope_did, proof} public knowledge.
    let packaged_proof = Proof {
        cdd_id: cdd_id,
        investor_did: raw_cdd_claim.investor_did,
        scope_id: scope_id,
        scope_did: raw_scope_claim.scope_did,
        proof,
    };
    let proof_str = serde_json::to_string(&packaged_proof)
        .unwrap_or_else(|error| panic!("Failed to serialize the proof: {}", error));

    if cfg.verbose {
        println!("Proof Package: {:?}", proof_str);
    }

    if let Some(p) = cfg.proof {
        std::fs::write(p, proof_str.as_bytes()).expect("Failed to write the proof to file.");
        println!("Successfully wrote the proof.");
    }
}

fn process_create_mocked_investor_uid(cfg: CreateMockedInvestorUidInfo) {
    // Sanitize Did input.
    let did = cfg.did.strip_prefix("0x").unwrap_or(&cfg.did);
    let did = did.chars().filter(|c| *c != '-').collect::<String>();
    let raw_did = hex::decode(did).expect("Invalid input DID, please use hex format");
    assert!(
        raw_did.len() == 32,
        "Invalid input DID, len should be 64 hex characters"
    );

    // Generate the mocked InvestorUid
    let investor_uid = mocked::make_investor_uid(&raw_did);
    if cfg.formatted {
        println!(
            "{}-{}-{}-{}-{}",
            hex::encode(&investor_uid[0..4]),
            hex::encode(&investor_uid[4..6]),
            hex::encode(&investor_uid[6..8]),
            hex::encode(&investor_uid[8..10]),
            hex::encode(&investor_uid[10..16])
        );
    } else {
        println!("{}", hex::encode(investor_uid));
    }
}

fn main() {
    let args: CLI = CLI::from_args();

    match args {
        CLI::CreateCDDId(cfg) => process_create_cdd_id(cfg),
        CLI::CreateClaimProof(cfg) => process_create_claim_proof(cfg),
        CLI::CreateMockedInvestorUid(cfg) => process_create_mocked_investor_uid(cfg),
    }
}
