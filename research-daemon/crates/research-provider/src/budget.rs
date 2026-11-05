use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

/// A per-job or per-process budget.
///
/// Queries are reserved before a request is sent; credits are recorded after
/// the provider reports them. The credit check is therefore slightly
/// conservative: a request may overshoot the credit ceiling by its own cost.
#[derive(Debug)]
pub struct Budget {
    max_queries: u32,
    max_credits: u64,
    queries_used: AtomicU32,
    credits_used: AtomicU64,
}

impl Budget {
    pub fn new(max_queries: u32, max_credits: u64) -> Self {
        Self {
            max_queries,
            max_credits,
            queries_used: AtomicU32::new(0),
            credits_used: AtomicU64::new(0),
        }
    }

    pub fn unlimited() -> Self {
        Self::new(u32::MAX, u64::MAX)
    }

    pub fn try_reserve_query(&self) -> Result<QueryReservation<'_>, BudgetExceeded> {
        if self.credits_used.load(Ordering::SeqCst) >= self.max_credits {
            return Err(BudgetExceeded::Credits {
                used: self.credits_used.load(Ordering::SeqCst),
                max: self.max_credits,
            });
        }

        let previous = self.queries_used.fetch_add(1, Ordering::SeqCst);
        if previous >= self.max_queries {
            self.queries_used.fetch_sub(1, Ordering::SeqCst);
            return Err(BudgetExceeded::Queries {
                used: previous,
                max: self.max_queries,
            });
        }

        Ok(QueryReservation {
            budget: self,
            committed: false,
        })
    }

    pub fn record_credits(&self, credits: u64) {
        self.credits_used.fetch_add(credits, Ordering::SeqCst);
    }

    pub fn queries_used(&self) -> u32 {
        self.queries_used.load(Ordering::SeqCst)
    }

    pub fn credits_used(&self) -> u64 {
        self.credits_used.load(Ordering::SeqCst)
    }

    pub fn max_queries(&self) -> u32 {
        self.max_queries
    }

    pub fn max_credits(&self) -> u64 {
        self.max_credits
    }
}

/// A reserved query slot. Dropping without `commit` refunds the slot.
#[derive(Debug)]
pub struct QueryReservation<'a> {
    budget: &'a Budget,
    committed: bool,
}

impl QueryReservation<'_> {
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for QueryReservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            self.budget.queries_used.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BudgetExceeded {
    #[error("query budget exhausted ({used}/{max})")]
    Queries { used: u32, max: u32 },
    #[error("credit budget exhausted ({used}/{max})")]
    Credits { used: u64, max: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_reservations_are_limited_and_refunded() {
        let budget = Budget::new(2, 100);

        let first = budget.try_reserve_query().unwrap();
        first.commit();
        assert_eq!(budget.queries_used(), 1);

        {
            let _second = budget.try_reserve_query().unwrap();
            assert_eq!(budget.queries_used(), 2);
        }
        assert_eq!(budget.queries_used(), 1);

        let second = budget.try_reserve_query().unwrap();
        second.commit();
        assert!(budget.try_reserve_query().is_err());
    }

    #[test]
    fn credits_gate_new_queries() {
        let budget = Budget::new(10, 5);
        budget.record_credits(5);
        assert!(budget.try_reserve_query().is_err());
    }
}
