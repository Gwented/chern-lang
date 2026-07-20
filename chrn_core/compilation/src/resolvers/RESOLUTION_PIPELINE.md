1. NameResolver
Goal: Registering all symbol namespaces.

2. MemberResolver:
Goal: Appends fields/variants and resolves their type.

3. TypeResolver
Goal: Resolves expressions, infers types, creates configs

4. ConstraintResolver
Goal: ?

NOTE: Since some resolvers are really just checkers, maybe make the distinction of
checkers and resolvers in naming.
