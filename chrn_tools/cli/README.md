# CLI

## Boop

## Extensions
Since the `chrn` base language is meant to be extended upon by using it's metadata from script files/blocks, the cli allows for extensions.

Any existent path variable with a binary executable that starts with "chrn-" will be executed.

Meaning, if there exists "chrn-json" and "chrn json" is typed, it will see that "chrn json" is not a valid argument and search for that binary matching the convention, which will lead to it being executed just like any other built-in argument.

To use this, there must exist a `CHRN_EXTENSIONS=1` path variable, otherwise it will not search for extensions. Extensions are not searched for by default due to possible arbitrary code execution with such a functionality.
