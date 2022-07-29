use std::convert::TryInto;

use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LookupMap, Vector},
    ext_contract, near_bindgen,
    setup_alloc, log, BorshStorageKey,
    serde::{Deserialize, Serialize},
    env, Promise, AccountId, PromiseResult,
    json_types::{ValidAccountId, U64, U128},
};

setup_alloc!();
pub const TGAS: u64 = 1_000_000_000_000;
pub const NO_DEPOSIT: u128 = 0;
pub const XCC_SUCCESS: u64 = 1;

#[ext_contract(ext_pair)]
pub trait NFT141Pair {
    fn init_vault(
        nft_contract_address: AccountId,
        vault_name: String,
        vault_symbol: String,
        feature_media: String
    );
    fn get_infos(self) -> PairInfos;
    fn setParams(
        &mut self,
        _name: String,
        _symbol: String,
        _value: U128,
        _media: String
    );
}

#[ext_contract(ext_self)]
pub trait SelfContract {
    fn pair_info_callback(&self) -> PairInfos;
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct PairInfos {
    pub name: String,
    pub symbol: String,
    pub supply: U128,
    pub media: String
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKeyEnum {
    NftToToken,
    IndexToNft,
    PairsInfo
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize)]
pub struct NFT141Factory {
    // keep track of nft address to pair address
    nft_to_token: LookupMap<AccountId, AccountId>,
    index_to_nft: LookupMap<u64, AccountId>,
    pairs_info: Vec<PairInfos>,
    counter: u64,
    fee: U128 
}

impl Default for NFT141Factory {
    fn default() -> Self {
        Self {
            nft_to_token: LookupMap::<AccountId, AccountId>::new(StorageKeyEnum::NftToToken),
            index_to_nft: LookupMap::<u64, AccountId>::new(StorageKeyEnum::IndexToNft),
            pairs_info: Vec::new(),
            counter: 0,
            fee: U128::from(0)
        }
    }
}

#[near_bindgen]
impl NFT141Factory {
    #[payable]
    pub fn nft141Pair(
        &mut self,
        name: String,
        nft_origin: AccountId,
        nft_symbol: String,
        feature_media: String
    ) {
        // assert valid nft origin contract address

        assert_eq!(self.nft_to_token.get(&nft_origin), None, "Found this contract address before");
        // Deploy pair contract
        let pair_contract = get_pair_contract_name(
            nft_symbol.clone()
        );
        Promise::new(pair_contract.clone())
            .create_account()
            .transfer(25_00000000000000000000000)
            .add_full_access_key(env::signer_account_pk())
            .deploy_contract(include_bytes!("../../nft141pair/res/nft141pair.wasm").to_vec());

        let owner: ValidAccountId = env::signer_account_id().try_into().unwrap();

        // Call pair contract constructor
        ext_pair::init_vault(
            nft_origin.clone(), 
            name.clone(), 
            nft_symbol.clone(),
            feature_media.clone(),
            &pair_contract,
            0,
            env::prepaid_gas() / 3
        );

        self.nft_to_token.insert(&nft_origin, &pair_contract);
        self.index_to_nft.insert(&self.counter, &nft_origin);
        self.counter += 1

        //emit event
    }

    pub fn getPairByNftAddress(&self, index: u64) {
        let _originalNft = self.index_to_nft.get(&index).unwrap();
        let _nft141pair = self.nft_to_token.get(&_originalNft).unwrap();
        ext_pair::get_infos(
            &_nft141pair,
            0,
            5 * TGAS
        )
        .then(ext_self::pair_info_callback(
            &env::current_account_id(), // this contract's account id
            0, // yocto NEAR to attach to the callback
            5 * TGAS // gas to attach to the callback
        ));
    }

    pub fn refreshAllPairsInfo(&mut self) {
        let mut i: u64 = 0;
        self.pairs_info.clear();
        while i < self.counter {
            self.getPairByNftAddress(i);
            i = i + 1;
        };
    }

    pub fn getAllPairsInfo(&self) -> Vec<PairInfos> {
        self.pairs_info.clone()
    }

    pub fn getPairAddressByIndex(&self, index: u64) -> AccountId {
        let _originalNft = self.index_to_nft.get(&index).unwrap();
        let _nft141pair = self.nft_to_token.get(&_originalNft).unwrap();
        _nft141pair
    }

    pub fn getCounter(&self) -> u64 {
        self.counter
    }

    // this is to sset value in case we decided to change tokens given to a tokenizing project.
    pub fn setValue(
        &mut self,
        _pair: AccountId,
        _name: String,
        _symbol: String,
        _value: U128,
        _media: String
    ) {
        //assert owner
        ext_pair::setParams(
            _name,
            _symbol,
            _value,
            _media,
            &_pair,
            0,
            env::prepaid_gas() / 2
        );
    }

    pub fn setFee(&mut self, _fee: U128) {
        //assert owner
        self.fee = _fee;
    }

    pub fn pair_info_callback(&mut self) -> PairInfos {
        assert_eq!(
            env::promise_results_count(),
            1,
            "This is a callback method"
        );
      
        match env::promise_result(0) {
          PromiseResult::NotReady => unreachable!(),
          PromiseResult::Failed => unreachable!(),
          PromiseResult::Successful(result) => {
            let info: PairInfos = near_sdk::serde_json::from_slice::<PairInfos>(&result).unwrap();
            self.pairs_info.push(info.clone());
            info
          }
        }
    }
}

fn get_pair_contract_name(_target: String) -> String {
    let prefix = _target.replace(".", "-");
    format!("{}.{}", prefix, env::current_account_id()).to_lowercase()
}

#[cfg(test)]
mod tests {
    // Testing boilerplate
    use super::*;
    use near_sdk::MockedBlockchain;
    use near_sdk::{testing_env, VMContext};

    const NFT_CONTRACT_ADDRESS: &'static str = "nft.yoshitoke.testnet";
    const NFT_MEDIA_URI: &'static str = "https://cdn-icons-png.flaticon.com/512/1137/1137074.png";

    // Context initializer function
    fn get_context(input: Vec<u8>, is_view: bool) -> VMContext {
        VMContext {
            current_account_id: "alice.testnet".to_string(),
            signer_account_id: "robert.testnet".to_string(),
            signer_account_pk: vec![0, 1, 2],
            predecessor_account_id: "jane.testnet".to_string(),
            input,
            block_index: 0,
            block_timestamp: 0,
            account_balance: 10u128.pow(25),
            account_locked_balance: 0,
            storage_usage: 0,
            attached_deposit: 0,
            prepaid_gas: 10u64.pow(18),
            random_seed: vec![0, 1, 2],
            is_view,
            output_data_receivers: vec![],
            epoch_height: 19,
        }
    }

    // Test cases here
    #[test]
    fn create_factory() {
        // Initialize context
        let context = get_context(vec![], false);
        testing_env!(context);

        // let target_nft_contract = "nft.testnet".to_string();
        // let nft_token_id = "0".to_string();

        let mut contract = NFT141Factory::default();

        contract.nft141Pair(
            "Yeti".into(), 
            NFT_CONTRACT_ADDRESS.into(), 
            "YTI".into(),
            NFT_MEDIA_URI.into()
        );

        // let promise = contract.getPairByNftAddress(0);
        assert_eq!(NFT_CONTRACT_ADDRESS, contract.index_to_nft.get(&0).unwrap());
        // let expected_shares_contract = get_shares_contract_name(target_nft_contract.clone(), nft_token_id.clone());

        // let saved_shares_address = contract.nft_to_shares_address.get(&nft_address);
        // let saved_nft_address = contract.shares_to_nft_address.get(&expected_shares_contract);

        // // Ensure that mappings are correctly saved
        // assert_eq!(saved_shares_address.expect("Saved shares address did not match"), expected_shares_contract);
        // assert_eq!(saved_nft_address.expect("Saved NFT address did not match"), nft_address);
    }

    #[test]
    fn pair_contract_grant_escrow_access() {

    }
}
