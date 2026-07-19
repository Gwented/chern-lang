use super::helpers::*;

#[test]
fn checked_consume_budget_tests() {
    // Overflow check
    let mut budget = MemoryBudget::default();
    budget.consume(1);
    assert!(matches!(
        budget.checked_consume(usize::MAX),
        BudgetResult::Overflow
    ));

    // Overage check
    let mut budget = MemoryBudget::new(10);
    assert!(matches!(
        budget.checked_consume(15),
        BudgetResult::Overage(5)
    ));

    // Should not have consumed anything since it was an overage
    assert_eq!(budget.remaining(), 10);

    // Limit Reached
    let mut budget = MemoryBudget::new(10);
    assert!(matches!(
        budget.checked_consume(10),
        BudgetResult::LimitReached,
    ));

    // Stable
    let mut budget = MemoryBudget::new(10);
    assert!(matches!(budget.checked_consume(9), BudgetResult::Stable,));
}

#[test]
fn reporter_budget_test() {
    let mut reporter = Reporter::new(5);
    let res = reporter.append_safe(&mut make_diagnostics(5));
    assert_eq!(
        res, true,
        "Should only be LimitReached which should not return `false`"
    );
}
