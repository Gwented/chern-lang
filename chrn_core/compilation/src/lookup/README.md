# NOTES

## scopes.rs

Section encoded scopes are based off of internal language philosophy reasons, nothing else.

Lookup preference is a concept that mimics the same behavior that would be seen from a scope system that may instead directly say something like, a variable symbol ids are in this scope and type symbol ids are in this, which allows for a built-in capability to allow for same name types, variables, aliases, etc.

The preference mimics this by taking by asking if the current symbol's kind is the preferred one, if not, then it stores that in-case it does not find the preferred symbol. This does NOT allow for same scope identifiers but it does allow for different section same identifiers because it will simply favor the correct one if found.

The objectively more user-friendly version of just making separate scopes which can have the same identifier wasn't chosen because it complicates internals more. No particular other reason, I don't actually think that's a good reason. See [link](../../../../IMPORTANT.txt)
