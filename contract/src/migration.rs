use crate::*;

#[near_bindgen]
impl SocialPremium {
    #[private]
    #[init(ignore_state)]
    #[allow(dead_code)]
    pub fn migrate(referral_fee: FeeFraction, premium_referral_fee: FeeFraction) -> Self {
        referral_fee.assert_valid();
        premium_referral_fee.assert_valid();

        #[derive(BorshDeserialize)]
        struct OldContract {
            owner_id: AccountId,
            subscriptions: UnorderedMap<SubscriptionName, VSubscription>,
            account_locks: LookupMap<AccountId, BlockHeight>,
            deposits: Balance,
            operations: u64,
        }

        let old_contract: OldContract = env::state_read().expect("Old state doesn't exist");

        Self {
            owner_id: old_contract.owner_id,
            subscriptions: old_contract.subscriptions,
            account_locks: old_contract.account_locks,
            deposits: old_contract.deposits,
            operations: old_contract.operations,
            referrals: UnorderedMap::new(StorageKey::Referrals),
            referral_fee,
            premium_referral_fee,
            referral_rewards: LookupMap::new(StorageKey::ReferralRewards),
            total_referral_rewards: 0,
        }
    }
}
