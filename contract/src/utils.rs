use crate::*;

impl SocialPremium {
    pub fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NO_ACCESS"
        );
    }

    pub fn lock_account(&mut self, account_id: &AccountId) {
        let account_lock: BlockHeight = self.account_locks.insert(account_id, &env::block_height()).unwrap_or(0);

        assert!(
            account_lock + BLOCKS_NUM_TO_LOCK_ACCOUNT <= env::block_height(),
            "ERR_ACCOUNT_LOCKED"
        );
    }

    pub fn assert_subscription(&self, subscription_name: &SubscriptionName) {
        assert!(self.subscriptions.get(subscription_name).is_some(), "ERR_NO_SUBSCRIPTION")
    }
}
