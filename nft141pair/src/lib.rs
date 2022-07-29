use std::convert::TryInto;

use near_contract_standards::fungible_token::FungibleToken;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, LazyOption}, Balance, PromiseOrValue,
    ext_contract, near_bindgen, PanicOnDefault,
    setup_alloc, log, BorshStorageKey,
    serde::{Deserialize, Serialize},
    env, Promise, AccountId,
    json_types::{ValidAccountId, U64, U128},
};
mod metadata;
use metadata::{NFT141PairMetadata, NFT141PairMetadataProvider, NFT141_FT_METADATA_SPEC};

setup_alloc!();

pub type TokenId = String;

#[ext_contract]
pub trait NonFungibleTokenCore {
    fn nft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        approval_id: Option<U64>,
        memo: Option<String>,
    );
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKeyEnum {
    FungibleToken,
    Metadata
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct NFT141Pair {
    pub token: FungibleToken,
    pub metadata: LazyOption<NFT141PairMetadata>,
    pub factory_contract_address: AccountId,
    pub nft_contract_address: AccountId,
    pub nft_value: U128,
    pub vault_name: String,
    pub vault_symbol: String,
    pub feature_media: String
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct PairInfos {
    pub name: String,
    pub symbol: String,
    pub supply: U128,
    pub media: String,
}

#[near_bindgen]
impl NFT141Pair {
    #[init]
    pub fn init_vault(
        nft_contract_address: AccountId,
        vault_name: String,
        vault_symbol: String,
        feature_media: String
    ) -> Self {
        // assert!(factory == address(0)); //Watch out TEST this is so we can init several time
        assert!(!env::state_exists(), "Already initialized");

        let metadata = NFT141PairMetadata {
            spec: NFT141_FT_METADATA_SPEC.to_string(),
            name: vault_name.to_string(),
            symbol: vault_symbol.to_string(),
            icon: Some(feature_media.to_string()),
            reference: None,
            reference_hash: None,
            decimals: 24
        };
        metadata.assert_valid();

        let mut this = Self {
            token: FungibleToken::new(StorageKeyEnum::FungibleToken),
            metadata: LazyOption::new(StorageKeyEnum::Metadata, Some(&metadata)),
            factory_contract_address: env::predecessor_account_id(),
            nft_contract_address,
            nft_value: U128::from(100 * 10u128.pow(24)),
            vault_name,
            vault_symbol,
            feature_media
        };
        
        // incentive
        this.token.internal_register_account(&env::current_account_id());
        this.token.internal_deposit(&env::current_account_id(), this.nft_value.0);

        this
    }

    pub fn get_infos(self) -> PairInfos {
        // Handle '0' supply value?
        PairInfos {
            name: self.vault_name,
            symbol: self.vault_symbol,
            supply: U128::from(self.token.total_supply / self.nft_value.0 - 1),
            media: self.feature_media
        }
    }

    pub fn get_nft_contract_address(self) -> AccountId {
        self.nft_contract_address
    }

    #[payable]
    pub fn swap171(&mut self, _in: String, _out: String) {
        // Check approved?

        // Performing swap
        non_fungible_token_core::nft_transfer(
            env::current_account_id().try_into().unwrap(),
            _in.clone(),
            None,
            None,
            &self.nft_contract_address,
            1,
            env::prepaid_gas() / 2
        );
        non_fungible_token_core::nft_transfer(
            env::signer_account_id().try_into().unwrap(),
            _out.clone(),
            None,
            None,
            &self.nft_contract_address,
            1,
            env::prepaid_gas() / 2
        );

        //Emit events
    }

    #[payable]
    pub fn multi_nft_deposits(
        &mut self,
        _ids: Vec<String>
    ) {

        let mut i: u64 = 0;
        while i < _ids.len() as u64 {
            let tokenId: &String = _ids.get(i as usize).unwrap();
            non_fungible_token_core::nft_transfer(
                env::current_account_id().clone().try_into().unwrap(),
                tokenId.clone(),
                None,
                None,
                &self.nft_contract_address,
                1,
                env::prepaid_gas() / 2
            );

            i = i + 1;
        }

        //Check success logs here
        //Start mingting NEP-141 token
        if self.token.accounts.get(&env::predecessor_account_id()) == None {
            self.token.internal_register_account(&env::predecessor_account_id());
        }
        self.token.internal_deposit(&env::predecessor_account_id(), _ids.len() as u128 * self.nft_value.0);
    }

    #[payable]
    pub fn withdraw(&mut self, _id: String) {
        // Check token balance in wallet
        let user_account = env::predecessor_account_id();
        let user_balance = self.ft_balance_of(user_account.clone().try_into().unwrap());
        assert!(&user_balance.0 >= &self.nft_value.0, "Token balance is smaller than the nft value");
        // Promise transfer here
        non_fungible_token_core::nft_transfer(
            user_account.clone().try_into().unwrap(),
            _id.clone(),
            None,
            None,
            &self.nft_contract_address,
            1,
            env::prepaid_gas() / 3
        );
        // Burn nep141 in wallet
        self.token.accounts.insert(&user_account, &(user_balance.0 - self.nft_value.0));
        self.token.total_supply -= &self.nft_value.0;
        self.on_tokens_burned(user_account.clone(), self.nft_value.0);
    }

    #[payable]
    pub fn batch_withdraw(&mut self, _ids: Vec<String>) {
        let user_balance = self.ft_balance_of(env::predecessor_account_id().try_into().unwrap());
        assert!(user_balance.0 >= self.nft_value.0 * _ids.len() as u128,  "Token balance is smaller than the nft batch value");

        let mut i: usize = 0;
        while i < _ids.len() {
            let tokenId: &String = _ids.get(i as usize).unwrap();
            non_fungible_token_core::nft_transfer(
                env::predecessor_account_id().try_into().unwrap(),
                tokenId.clone(),
                None,
                None,
                &self.nft_contract_address,
                1,
                env::prepaid_gas() / 2
            );

            i = i + 1;
        }

        // Burn nep141 in wallet
        self.token.accounts.insert(&env::predecessor_account_id(), &(user_balance.0 - self.nft_value.0 * _ids.len() as u128));
        self.token.total_supply -= self.nft_value.0 * _ids.len() as u128;
        self.on_tokens_burned(env::predecessor_account_id(), self.nft_value.0 * _ids.len() as u128);
    }

    pub fn setParams(
        &mut self,
        _name: String,
        _symbol: String,
        _value: U128,
        _media: String
    ) {
        assert_eq!(env::predecessor_account_id(), self.factory_contract_address, "!authorized");
        self.vault_name = _name;
        self.vault_symbol = _symbol;
        self.nft_value = _value;
        self.feature_media = _media;
    }

    fn on_account_closed(&mut self, account_id: AccountId, balance: Balance) {
        log!("Closed @{} with {}", account_id, balance);
    }

    fn on_tokens_burned(&mut self, account_id: AccountId, amount: Balance) {
        log!("Account @{} burned {}", account_id, amount);
    }
}

near_contract_standards::impl_fungible_token_core!(NFT141Pair, token, on_tokens_burned);
near_contract_standards::impl_fungible_token_storage!(NFT141Pair, token, on_account_closed);

#[near_bindgen]
impl NFT141PairMetadataProvider for NFT141Pair {
    fn ft_metadata(&self) -> NFT141PairMetadata {
        self.metadata.get().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::test_utils::{accounts, VMContextBuilder};
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, Balance};

    use super::*;

    const TOTAL_SUPPLY: Balance = 100_000_000_000_000_000_000_000_000;
    const NFT_CONTRACT_ADDRESS: &'static str = "nft.yoshitoke.testnet";
    const NFT_MEDIA_URI: &'static str = "https://cdn-icons-png.flaticon.com/512/1137/1137074.png";
    const NFT_SYMBOL: &'static str = "yti";

    fn get_context(predecessor_account_id: ValidAccountId) -> VMContextBuilder {
        let mut builder = VMContextBuilder::new();
        builder
            .current_account_id(accounts(0))
            .signer_account_id(predecessor_account_id.clone())
            .predecessor_account_id(predecessor_account_id);
        builder
    }

    #[test]
    fn test_new() {
        let mut context = get_context(accounts(1));
        testing_env!(context.build());

        let contract = NFT141Pair::init_vault(
            NFT_CONTRACT_ADDRESS.into(),
            "yeti".into(),
            NFT_SYMBOL.into(),
            NFT_MEDIA_URI.into()
        );
        testing_env!(context.is_view(true).build());

        assert_eq!(contract.ft_total_supply().0, TOTAL_SUPPLY);
        assert_eq!(contract.ft_balance_of(accounts(0)).0, TOTAL_SUPPLY);
    }
}
