use crate::*;

impl SocialPremium {
    pub fn assert_owner(&self) {
        assert_eq!(
            env::predecessor_account_id(),
            self.owner_id,
            "ERR_NO_ACCESS"
        );
    }
}
