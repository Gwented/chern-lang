// This is still needed even though TypeResolver and
// ConstraintResolver do a lot of checking innately
//
// An example would be typedef not restricting itself to
// ONLY concrete types, which is just wrong.
//
// This should also be an unbiased checker that doesn't try
// ANY inference and simply type checks
