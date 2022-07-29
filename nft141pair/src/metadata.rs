use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::json_types::{Base64VecU8, U128};
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::AccountId;

pub const NFT141_FT_METADATA_SPEC: &str = "nft141-ft-1.0.0";
pub type TokenId = String;

#[derive(BorshDeserialize, BorshSerialize, Clone, Deserialize, Serialize)]
#[serde(crate = "near_sdk::serde")]
pub struct NFT141PairMetadata {
    pub spec: String,
    pub name: String,
    pub symbol: String,
    pub icon: Option<String>,
    pub reference: Option<String>,
    pub reference_hash: Option<Base64VecU8>,
    pub decimals: u8
}

pub trait NFT141PairMetadataProvider {
    fn ft_metadata(&self) -> NFT141PairMetadata;
}

impl NFT141PairMetadata {
    pub fn assert_valid(&self) {
        assert_eq!(&self.spec, NFT141_FT_METADATA_SPEC);
        assert_eq!(self.reference.is_some(), self.reference_hash.is_some());
        if let Some(reference_hash) = &self.reference_hash {
            assert_eq!(reference_hash.0.len(), 32, "Hash has to be 32 bytes");
        }
    }
}
