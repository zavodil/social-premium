use crate::*;

impl SocialPremium {
    pub fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NO_ACCESS"
        );
    }

    pub fn assert_account_unlocked(&self, account_id: &AccountId) {
        let account_lock: BlockHeight = self.account_locks.get(account_id).unwrap_or(0);

        assert!(
            account_lock >= env::block_height() + BLOCKS_NUM_TO_LOCK_ACCOUNT,
            "ERR_ACCOUNT_LOCKED"
        );
    }

    pub fn assert_subscription(&self, subscription_name: &SubscriptionName) {
        assert!(self.subscriptions.get(subscription_name).is_some(), "ERR_NO_SUBSCRIPTION")
    }
}
