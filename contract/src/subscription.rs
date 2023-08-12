use crate::*;

#[derive(BorshSerialize, BorshDeserialize)]
pub enum VSubscription {
    Current(Subscription),
}

#[derive(BorshSerialize, BorshDeserialize)]
pub struct Subscription {
    pub title: String,
    pub description: String,
    pub image_url: String,
    pub price: u128,
    pub price_wholesale: u128,
}

impl From<VSubscription> for Subscription {
    fn from(v_subscription: VSubscription) -> Self {
        match v_subscription {
            VSubscription::Current(subscription) => subscription,
        }
    }
}

#[derive(BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SubscriptionOutput {
    pub title: String,
    pub description: String,
    pub image_url: String,
    pub price: U128,
    pub price_wholesale: U128,
}

impl From<VSubscription> for SubscriptionOutput {
    fn from(v_subscription: VSubscription) -> Self {
        match v_subscription {
            VSubscription::Current(subscription) => SubscriptionOutput {
                title: subscription.title,
                description: subscription.description,
                image_url: subscription.image_url,
                price: U128::from(subscription.price),
                price_wholesale: U128::from(subscription.price_wholesale),
            },
        }
    }
}
