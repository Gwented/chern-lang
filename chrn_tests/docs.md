// LSP in Go

## Goal
- To allow for instructions that state how to serialize data without something like macros or annotations. All features outside of this are entirely negligible.

## BEHAVIOR
- Ends program by default when type information is incorrect unless `#warn` is used.

- Binary representation. <-

## Types
i8, u8, i16, u16, i32, u32, i64, u64
i128, u128, f16, f32, f64, f128, sized, unsized,
char, bool, (maybe capital) str, struct, enum, tuple, nil, BigInt, BigFloat, List, Map, Set

`struct` for a structure of data.
`enum` for an Enum type which can also hold data.
`tuple`

## [Operators]
`!`: NOT operator.
`&&`: AND operator.
`||` OR operator.

## Keywords
// TODO:
`self`: Refers to current serialized data being looked at

`struct` for a structure of data.
`enum` for an Enum type which can also hold data.

## Actions (Ignore this)

```chrn

alias LongDefault(x, y) = [!IsEmpty, Range(x, y), StartsW("ch"), EndsW("ern"), Contains("chern")]

alias ShortDefault() = IsWhitespace

var->
    special_string: str [LongDefault(0, 5)]

    some_str: str [ShortDefault()]
```


# DOES NOT EXIST YET
`_`: Match all for ignoring parameters

```chrn
alias gopher(x, y) = !IsEmpty, Range(x = 0.0, y = 5.2), StartsW("ch") EndsW("ern") Contains("chern")

var->
    special_stir: str [gopher(0.5, _)] // defaults to (0.5, 5.0) 

    stirring: str [gopher(2.0, 5.0)] // Works as normal
```

`?`: Infers type and expects type consistency throughout entire given serialized data file type.
Maybe this should just mean it's original intent of, ignore the type.

`~`: Name bypass operator for when naming types.

Example:
```chrn
var->
    x: ~str
nest->
    struct ~str { // Could also just be "str" but it is best to maintain the prefix '~'
        ptr: u8
        len: unsized
        capacity: unsized
    }
```


# DOES NOT EXIST YET
`(range)`: Explicit range syntax. The '=' is required. `0..=5`

## [Predicates]
`IsEmpty`: Checks if the given array or string has a length of 0.

`IsWhitespace`: Checks if a string is only whitespace within UTF-8 standards, or is empty.

## Functions

// WHAT IF ALL OF THESE WORKED ON NUMBERS?

`Equals(Variadic)`: Checks serialized value for equality against given argument

`Range(inclusive, inclusive)`: Checks if the data being viewed matches the range given. For arrays and strings, this checks the length. For numbers, this checks the numeric value.

`Contains(DynType)`: Checks if the data being viewed contains the given literal.

`StartsW(DynType)`: Checks if the data being viewed starts with the given literal.

`EndsW(DynType)`:

// Does not exist yet
`Regex("0-9a-zA-Z*")`

## Statements

// TODO
`const`:

`export`:

`import`:

// TODO

`alias`: Allows for predicates to be stored within a single keyword in the case of long conditions.

```chrn
alias LongDefault(x, y) = !IsEmpty, Range(x, y), StartsW("ch") EndsW("ern") Contains("chern")

alias ShortDefault() = IsWhitespace

var->
special_string: str [LongDefault(0, 5)]

some_str: str [ShortDefault()]
```

`bind`: Defines where a serialized file is located that should be checked, or deserialized.

## [Sections]

// This sounds convoluted..
- Sections are how data can be parsed in different ways. They exist as opposed to keywords so that data is always defined in a readable, predictable manner.

- The `->` operator is used after section keywords to swap to the section. There cannot be more than one of each section.

`var`: Front facing definitions of the data to be serialized or deserialized.

```chrn
// If we have struct Person, it would look like
var->
    name: str
    age: u8

// But given nested data such as
    account: Account
// it would need a nest section

```

`nest->`: Allows for the definition of a struct or enum

```chrn
var->
    id: u64
    account: Account
    state: State
nest->
    struct Account {
        balance: BigFloat
    }

    enum State {
        Ready(str, unsized) // Can store tuple as well as no type
        InProgress
        Failed
    }

```

# DOES NOT EXIST YET
`override->`: What to default to when a language doesn't contain a particular type. Language defaults exist but this can change any if needed.

(Probably not a good idea)
There is also a "like" category. A "JAVA_LIKE" category would have all of the int, short, logic for a batch of languages.

`complex`: Define complex rules such as enum bounds.

    complex:
        State.variants = 5

## Arguments
`#warn`: Would warn instead of terminating.

//DOES NOT EXIST
`#ignore`: Ignores all errors and warns on the type this is applied to for serialized data related errors.

`#ign_if`: (Would remove anything that didn't align under condition rather than crash or warn.)

`#scient`, `#hex`, `#bin`, `#octo`: Numeric notations to output in serialized file.

#### Full example of language

```chrn
@def
    var->
        name: str
        age: u8 #warn #bin
        pets: List<Pet> [!IsEmpty, Range(5, 15)]
    nest->
        struct Pet {
            name: str [!IsWhitespace]
            color: Color
        }

        enum Color {Red(u8) Blue(u8) Green(u8) } #hex
@end
```

Utilities to alter actual main file, such as trimming all strings.

# I FORGOT ABOUT UNICODE
Allows for notation to serialize to be a specific notation. Unicode.

## POSSIBLE FEATURES

Maybe arithmetic
# Ok maybe

Matrix declarations.
HOW?

Unified serialization rules for any md file. 
Yaml, XML(Forgot this existed), Json, BINARY(I don't know) BINARY
