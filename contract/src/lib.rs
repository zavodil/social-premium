use near_sdk::json_types::U128;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::LookupMap,
    collections::UnorderedMap,
    env, ext_contract, log, near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Balance, BlockHeight, BorshStorageKey, Gas, PanicOnDefault, Promise, PromiseError,
    ONE_YOCTO,
};

const SOCIAL_DB_ACCOUNT_ID: &str = "social.near";
const SOCIAL_PREMIUM_ACCOUNT_ID: &str = "premium.social.near";
// const SOCIAL_DB_ACCOUNT_ID: &str = "v1.social08.testnet";
// const SOCIAL_PREMIUM_ACCOUNT_ID: &str = "test_alice.testnet";

const BLOCKS_NUM_TO_LOCK_ACCOUNT: BlockHeight = 60;

mod migration;
mod social;
mod subscription;
mod utils;

use crate::social::*;
use crate::subscription::*;
use crate::utils::FeeFraction;

type SubscriptionName = String;
type ReferralAccountId = AccountId;

const SOCIAL_PREMIUM_TREASURY_ACCOUNT_ID: &str = "treasury.premium.social.near";
const YEAR_IN_MS: u128 = 31556926000;

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Subscriptions,
    AccountLocks,
    Referrals,
    ReferralRewards,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct SocialPremium {
    owner_id: AccountId,
    // list of available subscriptions
    subscriptions: UnorderedMap<SubscriptionName, VSubscription>,
    // list of account locks to avoid callback collisions
    account_locks: LookupMap<AccountId, BlockHeight>,
    // total deposits
    deposits: Balance,
    // num of performed operation to buy premium
    operations: u64,
    // list of accounts and their last referrals
    referrals: UnorderedMap<AccountId, ReferralAccountId>,
    // referral fee for regular referrals
    referral_fee: FeeFraction,
    // referral fee for referrals with active premium
    premium_referral_fee: FeeFraction,
    // historical rewards for each referral
    referral_rewards: LookupMap<ReferralAccountId, Balance>,
    // total historical rewards
    total_referral_rewards: Balance,
}

#[near_bindgen]
impl SocialPremium {
    #[init]
    pub fn new(
        owner_id: AccountId,
        referral_fee: FeeFraction,
        premium_referral_fee: FeeFraction,
    ) -> Self {
        referral_fee.assert_valid();
        premium_referral_fee.assert_valid();

        Self {
            owner_id,
            subscriptions: UnorderedMap::new(StorageKey::Subscriptions),
            account_locks: LookupMap::new(StorageKey::AccountLocks),
            deposits: 0,
            operations: 0,
            referrals: UnorderedMap::new(StorageKey::Referrals),
            referral_fee,
            premium_referral_fee,
            referral_rewards: LookupMap::new(StorageKey::ReferralRewards),
            total_referral_rewards: 0,
        }
    }

    #[payable]
    pub fn purchase(
        &mut self,
        name: SubscriptionName,
        receiver_id: Option<AccountId>,
        referral_id: Option<ReferralAccountId>,
    ) -> Promise {
        let receiver_id = receiver_id.unwrap_or(env::predecessor_account_id());

        let referral_account_id = if let Some(referral_id) = referral_id {
            // referral id was provided in the request
            assert!(referral_id != receiver_id, "ERR_SELF_REFERRAL_NOT_ALLOWED");
            Some(referral_id)
        } else {
            // previously stored referral id
            self.referrals.get(&receiver_id)
        };

        let deposit = env::attached_deposit();
        assert!(deposit >= MIN_DEPOSIT, "Deposit {} required", MIN_DEPOSIT);

        self.lock_account(&receiver_id);
        self.assert_subscription(&name);

        let mut keys: Vec<String> = vec![format!(
            "{}/badge/{}/accounts/{}",
            SOCIAL_PREMIUM_ACCOUNT_ID, name, receiver_id
        )];

        if let Some(referral_id) = referral_account_id.clone() {
            // check if referral_id is premium
            keys.push(format!(
                "{}/badge/{}/accounts/{}",
                SOCIAL_PREMIUM_ACCOUNT_ID, name, referral_id
            ))
        }

        ext_social::ext(AccountId::new_unchecked(SOCIAL_DB_ACCOUNT_ID.to_string()))
            .with_static_gas(GAS_FOR_SOCIAL_GET)
            .get(keys, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_AFTER_SOCIAL_GET)
                    .purchase_after_social_get(
                        receiver_id.clone(),
                        name,
                        U128::from(deposit),
                        referral_account_id,
                    ),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_UNLOCK)
                    .unlock_accounts(vec![receiver_id]),
            )
    }

    #[payable]
    pub fn transfer(&mut self, name: SubscriptionName, receiver_id: AccountId) -> Promise {
        assert_eq!(env::attached_deposit(), ONE_YOCTO, "ERR_ONE_YOCTO_REQUIRED");

        let sender_id = env::predecessor_account_id();

        self.assert_subscription(&name);

        assert_ne!(receiver_id, sender_id, "ERR_SENDER_IS_RECEIVER");
        self.lock_account(&receiver_id);
        self.lock_account(&sender_id);

        let keys: Vec<String> = vec![
            format!(
                "{}/badge/{}/accounts/{}",
                SOCIAL_PREMIUM_ACCOUNT_ID, name, sender_id
            ),
            format!(
                "{}/badge/{}/accounts/{}",
                SOCIAL_PREMIUM_ACCOUNT_ID, name, receiver_id
            ),
        ];

        ext_social::ext(AccountId::new_unchecked(SOCIAL_DB_ACCOUNT_ID.to_string()))
            .with_static_gas(GAS_FOR_SOCIAL_GET)
            .get(keys, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_AFTER_SOCIAL_GET)
                    .transfer_after_social_get(sender_id.clone(), receiver_id.clone(), name),
            )
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_UNLOCK)
                    .unlock_accounts(vec![sender_id, receiver_id]),
            )
    }

    pub fn set_referral_fee(&mut self, referral_fee: FeeFraction) {
        self.assert_owner();
        referral_fee.assert_valid();
        self.referral_fee = referral_fee;
    }

    pub fn set_premium_referral_fee(&mut self, premium_referral_fee: FeeFraction) {
        self.assert_owner();
        premium_referral_fee.assert_valid();
        self.premium_referral_fee = premium_referral_fee;
    }

    pub fn add_subscription(
        &mut self,
        name: SubscriptionName,
        title: String,
        description: String,
        image_url: String,
        price: U128,
        price_wholesale: U128,
    ) {
        self.assert_owner();

        let subscription = Subscription {
            title,
            description,
            image_url,
            price: price.0,
            price_wholesale: price_wholesale.0,
        };

        self.subscriptions
            .insert(&name, &VSubscription::Current(subscription));

        self.internal_set_subscription(name);
    }

    pub fn get_subscription(&self, name: SubscriptionName) -> SubscriptionOutput {
        self.subscriptions
            .get(&name)
            .expect("ERR_NO_SUBSCRIPTION")
            .into()
    }

    pub fn get_referral_reward(&self, referral_account_id: ReferralAccountId) -> U128 {
        U128::from(
            self.referral_rewards
                .get(&referral_account_id)
                .unwrap_or_default(),
        )
    }

    pub fn get_total_referral_rewards(&self) -> U128 {
        U128::from(self.total_referral_rewards)
    }

    pub fn get_affiliates(&self, referral_account_id: ReferralAccountId) -> Vec<AccountId> {
        self.referrals
            .into_iter()
            .filter(|(_, _referral_account_id)| (referral_account_id == *_referral_account_id))
            .map(|(user_account_id, _)| user_account_id)
            .collect()
    }

    pub fn get_deposits(&self) -> U128 {
        U128::from(self.deposits)
    }

    pub fn get_operations(&self) -> u64 {
        self.operations
    }

    pub fn get_lock(&self, account_id: AccountId) -> Option<BlockHeight> {
        self.account_locks.get(&account_id)
    }

    pub fn get_referral_id(&self, account_id: AccountId) -> Option<AccountId> {
        self.referrals.get(&account_id)
    }

    pub fn get_referral_fee(&self) -> FeeFraction {
        self.referral_fee.clone()
    }

    pub fn get_premium_referral_fee(&self) -> FeeFraction {
        self.premium_referral_fee.clone()
    }

    pub fn withdraw_deposits(
        &mut self,
        amount: U128,
        destination_account_id: Option<AccountId>,
    ) -> Promise {
        self.assert_owner();

        assert!(self.deposits >= amount.0, "ERR_NOT_ENOUGH_DEPOSITS");

        self.deposits -= amount.0;

        let destination_account_id = destination_account_id.unwrap_or(AccountId::new_unchecked(
            SOCIAL_PREMIUM_TREASURY_ACCOUNT_ID.to_string(),
        ));

        Promise::new(destination_account_id).transfer(amount.0)
    }

    pub fn get_purchase_ms(&self, name: SubscriptionName, amount: U128) -> U128 {
        let subscription = self.internal_get_subscription(&name);
        U128::from(self.get_subscription_purchased_period_ms(&subscription, amount.0))
    }
}

impl SocialPremium {
    pub(crate) fn internal_get_subscription(
        &self,
        subscription_name: &SubscriptionName,
    ) -> Subscription {
        Subscription::from(
            self.subscriptions
                .get(subscription_name)
                .expect("ERR_SUBSCRIPTION_NOT_FOUND"),
        )
    }

    pub(crate) fn internal_unlock_account(&mut self, account_id: &AccountId) {
        self.account_locks.remove(account_id);
    }

    fn get_subscription_price(&self, subscription: &Subscription, is_wholesale: bool) -> u128 {
        if is_wholesale {
            subscription.price_wholesale
        } else {
            subscription.price
        }
    }

    pub fn get_subscription_purchased_period_ms(
        &self,
        subscription: &Subscription,
        amount: u128,
    ) -> u128 {
        let price =
            self.get_subscription_price(subscription, amount >= subscription.price_wholesale);

        (U256::from(amount) * U256::from(YEAR_IN_MS) / U256::from(price)).as_u128()
    }
}

use uint::construct_uint;
construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
