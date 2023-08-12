use crate::*;

use near_sdk::serde_json::{Map, Value};

pub const GAS_FOR_SOCIAL_GET: Gas = Gas(Gas::ONE_TERA.0 * 10);
pub const GAS_FOR_SOCIAL_SET: Gas = Gas(Gas::ONE_TERA.0 * 40);
pub const GAS_FOR_AFTER_SOCIAL_GET: Gas = Gas(Gas::ONE_TERA.0 * 80);
pub const DEPOSIT_FOR_SOCIAL_SET: Balance = 50_000_000_000_000_000_000_000;
pub const MIN_DEPOSIT: Balance = 1_000_000_000_000_000_000_000_000;


#[derive(Serialize, Deserialize, Default)]
#[serde(crate = "near_sdk::serde")]
pub struct GetOptions {
    pub with_block_height: Option<bool>,
    pub with_node_id: Option<bool>,
    pub return_deleted: Option<bool>,
}

#[ext_contract(ext_social)]
pub trait ExtSocial {
    fn get(self, keys: Vec<String>, options: Option<GetOptions>) -> Value;
    fn set(&mut self, data: Value);
}

#[ext_contract(ext_self)]
pub trait ExtSocialPremium {
    fn after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        receiver_id: AccountId,
        subscription_name: String,
        amount: U128,
    );
}

#[near_bindgen]
impl SocialPremium {


    #[payable]
    #[private]
    pub fn after_social_get(
        &mut self,
        #[callback_result] value: Result<Value, PromiseError>,
        receiver_id: AccountId,
        subscription_name: SubscriptionName,
        amount: U128,
    ) {
        if let Ok(mut value) = value {
            let keys = value.as_object_mut().expect("Data is not a JSON object");

            let now: u128 = env::block_timestamp_ms().into();

            let paid_until: u128 = if keys.is_empty() {
                now
            } else {
                let badge = value.get(SOCIAL_PREMIUM_ACCOUNT_ID.to_string()).expect("ERR_NO_DATA");
                let subscriptions = badge.get("badge".to_string()).expect("ERR_NO_DATA");
                let subscription = subscriptions.get(subscription_name.to_string()).expect("ERR_NO_DATA");
                let accounts = subscription.get("accounts".to_string()).expect("ERR_NO_DATA");
                let paid_until = accounts.get(receiver_id.to_string()).expect("ERR_NO_DATA").as_str().unwrap().to_string();

                paid_until.parse::<u128>().unwrap()
            };

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
                receiver_id,
                subscription_timestamp,
            );
        }
    }
}

impl SocialPremium {
    fn internal_set_subscription_holder(
        &mut self,
        subscription_name: SubscriptionName,
        receiver_id: AccountId,
        timestamp: u128,
    ) {
        let mut receiver_data: Map<String, Value> = Map::new();
        receiver_data.insert(
            receiver_id.to_string(),
            Value::String(timestamp.to_string()),
        );

        let mut accounts_data: Map<String, Value> = Map::new();
        accounts_data.insert("accounts".to_string(), Value::Object(receiver_data));

        let mut subscription_data: Map<String, Value> = Map::new();
        subscription_data.insert(subscription_name, Value::Object(accounts_data));

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
            .set(Value::Object(data));
    }

    pub fn internal_set_subscription(&mut self, subscription_name: SubscriptionName) {
        let subscription = self.internal_get_subscription(&subscription_name);

        let mut image_data: Map<String, Value> = Map::new();
        image_data.insert("url".to_string(), Value::String(subscription.image_url));

        let mut badge_data: Map<String, Value> = Map::new();
        badge_data.insert(
            "title".to_string(),
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
            .set(Value::Object(data));
    }
}

