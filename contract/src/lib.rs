use near_sdk::json_types::U128;
use near_sdk::{
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::UnorderedMap,
    env, ext_contract, near_bindgen,
    serde::{Deserialize, Serialize},
    AccountId, Balance, BorshStorageKey, Gas, PanicOnDefault, Promise, PromiseError,
};

const SOCIAL_DB_ACCOUNT_ID: &str = "social.near";
const SOCIAL_PREMIUM_ACCOUNT_ID: &str = "premium.social.near";

mod social;
mod subscription;
mod utils;

use crate::subscription::*;
use crate::social::*;

type SubscriptionName = String;

const SOCIAL_PREMIUM_TREASURY_ACCOUNT_ID: &str = "treasury.premium.social.near";
const YEAR_IN_MS: u128 = 31556926000;

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    Subscriptions,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct SocialPremium {
    owner_id: AccountId,
    subscriptions: UnorderedMap<SubscriptionName, VSubscription>,
    deposits: Balance,
    operations: u64,
}

#[near_bindgen]
impl SocialPremium {
    #[init]
    pub fn new(owner_id: AccountId) -> Self {
        Self {
            owner_id,
            subscriptions: UnorderedMap::new(StorageKey::Subscriptions),
            deposits: 0,
            operations: 0,
        }
    }

    #[payable]
    pub fn purchase(&mut self, name: String, receiver_id: Option<AccountId>) -> Promise {
        let receiver_id = receiver_id.unwrap_or(env::predecessor_account_id());
        let deposit = env::attached_deposit();
        assert!(deposit >= MIN_DEPOSIT, "Deposit {} required", MIN_DEPOSIT);

        let keys: Vec<String> = vec![format!(
            "{}/badge/{}/accounts/{}",
            SOCIAL_PREMIUM_ACCOUNT_ID, name, receiver_id
        )];

        ext_social::ext(AccountId::new_unchecked(SOCIAL_DB_ACCOUNT_ID.to_string()))
            .with_static_gas(GAS_FOR_SOCIAL_GET)
            .get(keys, None)
            .then(
                ext_self::ext(env::current_account_id())
                    .with_static_gas(GAS_FOR_AFTER_SOCIAL_GET)
                    .after_social_get(receiver_id, name, U128::from(deposit)),
            )
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

    pub fn get_deposits(&self) -> U128 {
        U128::from(self.deposits)
    }

    pub fn get_operations(&self) -> u64 {
        self.operations
    }

    pub fn withdraw_deposits(&mut self, amount: U128) -> Promise {
        self.assert_owner();

        assert!(self.deposits >= amount.0, "ERR_NOT_ENOUGH_DEPOSITS");

        self.deposits -= amount.0;

        Promise::new(AccountId::new_unchecked(
            SOCIAL_PREMIUM_TREASURY_ACCOUNT_ID.to_string(),
        ))
        .transfer(amount.0)
    }

    pub fn get_purchase_ms(&self, name: SubscriptionName, amount: U128) -> U128 {
        let subscription = self.internal_get_subscription(&name);
        U128::from(self.get_subscription_purchased_period_ms(&subscription, amount.0))
    }
}

impl SocialPremium {
    pub(crate) fn internal_get_subscription(&self, subscription_name: &SubscriptionName) -> Subscription {
        Subscription::from(
            self.subscriptions
                .get(subscription_name)
                .expect("ERR_SUBSCRIPTION_NOT_FOUND"),
        )
    }

    fn get_subscription_price(&self, subscription: &Subscription, is_wholesale: bool) -> u128 {
        if is_wholesale {
            subscription.price
        } else {
            subscription.price_wholesale
        }
    }

    pub fn get_subscription_purchased_period_ms(
        &self,
        subscription: &Subscription,
        amount: u128,
    ) -> u128 {
        let price =
            self.get_subscription_price(subscription, amount > subscription.price_wholesale);

        (U256::from(amount) * U256::from(YEAR_IN_MS) / U256::from(price)).as_u128()
    }
}

use uint::construct_uint;
construct_uint! {
    /// 256-bit unsigned integer.
    pub struct U256(4);
}
