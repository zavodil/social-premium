use crate::*;

use near_sdk::serde_json::{Map, Value};

pub const GAS_FOR_SOCIAL_GET: Gas = Gas(Gas::ONE_TERA.0 * 10);
pub const GAS_FOR_SOCIAL_SET: Gas = Gas(Gas::ONE_TERA.0 * 40);
pub const GAS_FOR_AFTER_SOCIAL_GET: Gas = Gas(Gas::ONE_TERA.0 * 85);
pub const GAS_FOR_UNLOCK: Gas = Gas(Gas::ONE_TERA.0 * 10);
pub const DEPOSIT_FOR_SOCIAL_SET: Balance = 50_000_000_000_000_000_000_000;
pub const MIN_DEPOSIT: Balance = 1_000_000_000_000_000_000_000_000;

#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct GetOptions {
    pub with_block_height: Option<bool>,
    pub with_node_id: Option<bool>,
    pub return_deleted: Option<bool>,
}

#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct SetOptions {
    pub refund_unused_deposit: bool,
}

#[ext_contract(ext_social)]
pub trait ExtSocial {
    fn get(self, keys: Vec<String>, options: Option<GetOptions>) -> Value;
    fn set(&mut self, data: Value, options: SetOptions);
}

#[ext_contract(ext_self)]
pub trait ExtSocialPremium {
    fn purchase_after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        receiver_id: AccountId,
        subscription_name: String,
        amount: U128,
        referral_account_id: Option<ReferralAccountId>,
    );

    fn transfer_after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        sender_id: AccountId,
        receiver_id: AccountId,
        subscription_name: String,
    );

    fn unlock_accounts(&mut self, accounts: Vec<AccountId>);
}

#[near_bindgen]
impl SocialPremium {
    #[private]
    pub fn unlock_accounts(&mut self, accounts: Vec<AccountId>) {
        for account_id in accounts {
            self.internal_unlock_account(&account_id);
        }
    }

    #[private]
    pub fn purchase_after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        receiver_id: AccountId,
        subscription_name: SubscriptionName,
        amount: U128,
        referral_account_id: Option<ReferralAccountId>,
    ) {
        if let Ok(mut value) = value {
            let keys = value.as_object_mut().expect("Data is not a JSON object");

            let mut referral_is_premium = false;

            let now: u128 = env::block_timestamp_ms().into();

            let paid_until: u128 = if keys.is_empty() {
                now
            } else {
                let badge = value
                    .get(SOCIAL_PREMIUM_ACCOUNT_ID.to_string())
                    .expect("ERR_NO_DATA");
                let subscriptions = badge.get("badge".to_string()).expect("ERR_NO_DATA");
                let subscription = subscriptions
                    .get(subscription_name.to_string())
                    .expect("ERR_NO_DATA");
                let accounts = subscription
                    .get("accounts".to_string())
                    .expect("ERR_NO_DATA");
                let paid_until = accounts
                    .get(receiver_id.to_string())
                    .unwrap_or( &Value::from(now))
                    .as_str()
                    .unwrap_or_default()
                    .to_string();

                if let Some(referral_id) = referral_account_id.clone() {
                    let referral_paid_until =
                        if let Some(referral_paid_until_value) = accounts.get(referral_id.to_string()) {
                            referral_paid_until_value
                                .as_str()
                                .unwrap_or_default()
                                .parse::<u128>()
                                .unwrap()
                        } else {
                            0
                        };

                    referral_is_premium = referral_paid_until > now;
                }

                paid_until.parse::<u128>().unwrap_or(now)
            };

            // store affiliate reward
            if let Some(user_referral_id) = referral_account_id {
                self.referrals.insert(&receiver_id, &user_referral_id);

                let prev_referral_reward = self
                    .referral_rewards
                    .get(&user_referral_id)
                    .unwrap_or_default();
                let referral_reward = if referral_is_premium {
                    self.premium_referral_fee.multiply(amount.0)
                } else {
                    self.referral_fee.multiply(amount.0)
                };
                self.referral_rewards
                    .insert(&user_referral_id, &(prev_referral_reward + referral_reward));
                self.total_referral_rewards += referral_reward;

                log!(
                    "{}Referral reward for {}: {} yNEAR",
                    if referral_is_premium { "Premium " } else { "" },
                    user_referral_id,
                    referral_reward.to_string()
                );

                Promise::new(user_referral_id).transfer(referral_reward);
            }

            let subscription = self.internal_get_subscription(&subscription_name);

            let previously_purchased_ms = if paid_until > now {
                paid_until - now
            } else {
                0
            };

            let purchased_period_ms =
                self.get_subscription_purchased_period_ms(&subscription, amount.0);

            let subscription_timestamp = now + purchased_period_ms + previously_purchased_ms;

            self.deposits += amount.0;
            self.operations += 1;

            self.internal_set_subscription_holder(
                subscription_name,
                vec![SubscriptionData {
                    receiver_id,
                    timestamp: subscription_timestamp,
                }],
            )
            .as_return();
        }
    }

    pub fn transfer_after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        sender_id: AccountId,
        receiver_id: AccountId,
        subscription_name: SubscriptionName,
    ) {
        if let Ok(mut value) = value {
            let keys = value.as_object_mut().expect("Data is not a JSON object");

            let now: u128 = env::block_timestamp_ms().into();
            let now_string = now.to_string();

            if keys.is_empty() {
                panic!("ERR_SENDER_SUBSCRIPTION_NOT_FOUND");
            } else {
                let badge = value
                    .get(SOCIAL_PREMIUM_ACCOUNT_ID.to_string())
                    .expect("ERR_NO_DATA");
                let subscriptions = badge.get("badge".to_string()).expect("ERR_NO_DATA");
                let subscription = subscriptions
                    .get(subscription_name.to_string())
                    .expect("ERR_NO_DATA");
                let accounts = subscription
                    .get("accounts".to_string())
                    .expect("ERR_NO_DATA");

                let sender_paid_until_str =
                    if let Some(paid_until) = accounts.get(sender_id.to_string()) {
                        paid_until.as_str().unwrap()
                    } else {
                        panic!("ERR_SENDER_SUBSCRIPTION_NOT_FOUND")
                    };

                let receiver_paid_until_str =
                    if let Some(paid_until) = accounts.get(receiver_id.to_string()) {
                        paid_until.as_str().unwrap()
                    } else {
                        now_string.as_str()
                    };

                let sender_paid_until = sender_paid_until_str.to_string().parse::<u128>().unwrap();
                let receiver_paid_until = std::cmp::max(
                    now,
                    receiver_paid_until_str.to_string().parse::<u128>().unwrap(),
                );

                assert!(sender_paid_until > now, "ERR_SENDER_SUBSCRIPTION_NOT_FOUND");

                let sender_previous_purchased_ms = sender_paid_until - now;

                let receiver_timestamp = sender_previous_purchased_ms + receiver_paid_until;

                self.operations += 1;

                self.internal_set_subscription_holder(
                    subscription_name,
                    vec![
                        SubscriptionData {
                            receiver_id: sender_id,
                            timestamp: now,
                        },
                        SubscriptionData {
                            receiver_id,
                            timestamp: receiver_timestamp,
                        },
                    ],
                )
                .as_return();
            }
        }
    }
}

struct SubscriptionData {
    receiver_id: AccountId,
    timestamp: u128,
}

impl SocialPremium {
    fn internal_set_subscription_holder(
        &mut self,
        subscription_name: SubscriptionName,
        subscriptions: Vec<SubscriptionData>,
    ) -> Promise {
        let mut data: Map<String, Value> = Map::new();

        let badge_data = get_badge_data(&subscription_name, subscriptions);
        data.insert(
            SOCIAL_PREMIUM_ACCOUNT_ID.to_string(),
            Value::Object(badge_data),
        );

        ext_social::ext(AccountId::new_unchecked(SOCIAL_DB_ACCOUNT_ID.to_string()))
            .with_static_gas(GAS_FOR_SOCIAL_SET)
            .with_attached_deposit(DEPOSIT_FOR_SOCIAL_SET)
            .set(
                Value::Object(data),
                SetOptions {
                    refund_unused_deposit: true,
                },
            )
    }

    pub fn internal_set_subscription(&mut self, subscription_name: SubscriptionName) {
        let subscription = self.internal_get_subscription(&subscription_name);

        let mut image_data: Map<String, Value> = Map::new();
        image_data.insert("url".to_string(), Value::String(subscription.image_url));

        let mut badge_data: Map<String, Value> = Map::new();
        badge_data.insert(
            "name".to_string(),
            Value::String(subscription.title.to_string()),
        );
        badge_data.insert(
            "description".to_string(),
            Value::String(subscription.description.to_string()),
        );
        badge_data.insert(
            "price".to_string(),
            Value::String(subscription.price.to_string()),
        );
        badge_data.insert(
            "price_wholesale".to_string(),
            Value::String(subscription.price_wholesale.to_string()),
        );
        badge_data.insert("image".to_string(), Value::Object(image_data));

        let mut metadata_data: Map<String, Value> = Map::new();
        metadata_data.insert("metadata".to_string(), Value::Object(badge_data));

        let mut subscription_data: Map<String, Value> = Map::new();
        subscription_data.insert(subscription_name, Value::Object(metadata_data));

        let mut badge_data: Map<String, Value> = Map::new();
        badge_data.insert("badge".to_string(), Value::Object(subscription_data));

        let mut data: Map<String, Value> = Map::new();
        data.insert(
            SOCIAL_PREMIUM_ACCOUNT_ID.to_string(),
            Value::Object(badge_data),
        );

        ext_social::ext(AccountId::new_unchecked(SOCIAL_DB_ACCOUNT_ID.to_string()))
            .with_static_gas(GAS_FOR_SOCIAL_SET)
            .with_attached_deposit(DEPOSIT_FOR_SOCIAL_SET)
            .set(
                Value::Object(data),
                SetOptions {
                    refund_unused_deposit: true,
                },
            );
    }
}

fn get_badge_data(
    subscription_name: &String,
    subscriptions: Vec<SubscriptionData>,
) -> Map<String, Value> {
    let mut receiver_data: Map<String, Value> = Map::new();
    for subscription in subscriptions {
        receiver_data.insert(
            subscription.receiver_id.to_string(),
            Value::String(subscription.timestamp.to_string()),
        );
    }

    let mut accounts_data: Map<String, Value> = Map::new();
    accounts_data.insert("accounts".to_string(), Value::Object(receiver_data));

    let mut subscription_data: Map<String, Value> = Map::new();
    subscription_data.insert(subscription_name.to_owned(), Value::Object(accounts_data));

    let mut badge_data: Map<String, Value> = Map::new();
    badge_data.insert("badge".to_string(), Value::Object(subscription_data));

    badge_data
}
