//TODO: Either:
// This will stay how it is and enforce that users decide what to do
// A trait bound will be used which innately knows how to control itself, like, this budget requires
// a type so that it can automatically consume based off the time, but this budget consumes based
// off of the given data at face value.
/// Generic struct for holding a given budget and ensuring it doesnt go over the limit
#[derive(Debug)]
pub struct MemoryBudget {
    // u32?
    /// Amount that the usage cannot be greater than
    pub limit: usize,
    /// Metric that must be less than `self.limit`
    pub usage: usize,
    /// Times limit was exceeded
    pub times_exceeded: usize,
    /// If the budget was exceeded at any point, all future usage adds will increment this counter
    pub amt_exceeded: usize,
}

impl MemoryBudget {
    pub const fn new(limit: usize) -> MemoryBudget {
        MemoryBudget {
            limit,
            times_exceeded: 0,
            usage: 0,
            amt_exceeded: 0,
        }
    }

    /// - to_apply: Size to apply to usage
    ///
    /// Returns `BudgetResult` which represents all possible cases of this method's result
    ///
    /// * Notable behavior:
    /// If the limit was exceeded, `self.usage` remains the same, but the overage is returned.
    pub fn checked_consume(&mut self, to_apply: usize) -> BudgetResult {
        //SAFETY
        if let Some(proposed_sum) = self.usage.checked_add(to_apply) {
            if self.would_exceed(proposed_sum) {
                let overage = proposed_sum - self.limit;
                self.times_exceeded += 1;
                self.amt_exceeded = self.amt_exceeded.saturating_add(overage);

                return BudgetResult::Overage(overage);
            } else {
                self.usage = proposed_sum;

                if self.usage == self.limit {
                    return BudgetResult::LimitReached;
                }

                BudgetResult::Stable
            }
        } else {
            self.times_exceeded += 1;
            // Should this be done?
            self.amt_exceeded = usize::MAX;
            // Denoting that it was an overflow
            BudgetResult::Overflow
        }
    }

    //// Assumes the consumption operation won't exceed
    // pub fn consume(&mut self, to_apply: usize) -> bool {
    // self.usage -= to_apply;
    // if let Some(proposed_sum) = self.usage.checked_add(to_apply) {
    //     if self.would_exceed(proposed_sum) {
    //         let overage = proposed_sum - self.limit;
    //         self.times_exceeded += 1;
    //         self.amt_exceeded = self.amt_exceeded.saturating_add(overage);
    //         return BudgetResult::Overage(overage);
    //     } else {
    //         BudgetResult::Stable
    //     }
    // } else {
    //     self.times_exceeded += 1;
    //     // Should this be done?
    //     self.amt_exceeded = usize::MAX;
    //     // Denoting that it was an overflow
    //     BudgetResult::Overflow
    // }
    // }

    // fn increment_times_exceeded(&mut self) {
    //     self.times_exceeded += 1;
    // }

    //FIX:?
    /// - to_apply: Amount to remove from `self.usage`
    ///
    /// `Ok` means the operation was successful
    /// `Err` means an underflow occurred. Contains the amount underflown.
    ///
    /// On `Err` `self.usage` is set to 0 by default
    pub fn checked_remove(&mut self, to_apply: usize) -> Result<(), usize> {
        if let Some(difference) = self.usage.checked_sub(to_apply) {
            self.usage = difference;
            return Ok(());
        }

        // If I'm not hallucinating this should be safe since if to_apply is provably bigger than
        // self.usage, and to_apply can fit into a usize, which means this is fine
        let underflow = to_apply - self.usage;
        self.usage = 0;
        Err(underflow)
    }

    /// Sets `self.usage` to `self.limit`
    ///
    /// A use case would be an overage occuring, since `MemoryBudget` does not make any assumptions and keeps
    /// it's usage as what it was before the overage, only returning the overage. If the data being
    /// tracked can be externally used to perhaps use the amount right until it reaches the overage,
    /// this would be useful to just set it to the limit manually.
    pub fn set_to_limit(&mut self) {
        self.usage = self.limit;
    }

    // DO NOT QUESTION THIS
    /// Checks if `proposed` > `self.limit`
    pub fn would_exceed(&self, proposed: usize) -> bool {
        proposed > self.limit
    }
}

// Should this even exist? This is mostly for helping tests run more easily since they more likely
// than not don't care about the budget, but maybe that should be delegated to a local function
// inside of tests.
/// 4 MB
const DEFAULT_LIMIT: usize = (1024 * 1024) * 4;

impl Default for MemoryBudget {
    fn default() -> Self {
        Self {
            limit: DEFAULT_LIMIT,
            usage: Default::default(),
            amt_exceeded: Default::default(),
            times_exceeded: Default::default(),
        }
    }
}

// If usage exceeds the limit, but the amount it exceeded doesn't equal the limit, a different enum
// is returned to say, the limit was exceeded before this, but if you still want the amount that
// would be needed to fill the usage completely
/// This exists to cover the ambiguity of overage
pub enum BudgetResult {
    /// This is equivalent to `Ok` meaning no limit was reached and no error was had
    Stable,
    /// The limit exceeded with an overage. Contains the overage and how much of it's usage would be
    /// needed
    Overage(usize),
    /// If the usage is added to, and it is equal to the limit, this variant is reached.
    ///
    /// This definitively means that there is nothing more to be added to usage, and there was no overflow.
    LimitReached,
    /// The usage exceeds the limit, but overage can't be returned due to the overage exceeding `usize::MAX`
    Overflow,
}

// Maybe some types should implement a cost method under a trait where they can get their heap
// allocation amount as well?
// So cost trait?

//// Trait for allowing budgets to be calculated differently under a common type
// pub trait Budgetable {
//     fn checked_consume(&mut self, given: usize) -> Result<(), Option<usize>>;
//     fn checked_remove(&mut self, given: usize) -> Result<(), usize>;
// }

// pub struct TypedMemoryBudget<T> {
//     memory_budget: MemoryBudget,
//     /// For keeping the type internally
//     phantom_data: PhantomData<T>,
// }
// impl Budgetable for TypedMemoryBudget {}
//
// But what if this was just a function where the caller specifies a generic and it just wraps
// around the existing memory budget?
// pub fn typed_add<T>(mem_budget: MemoryBudget) {}
