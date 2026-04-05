// LSP in Go

## Goal
- To allow for instructions that state how to serialize data without something like macros or annotations. All features outside of this are entirely negligible.

## BEHAVIOR
- Ends program by default when type information is incorrect unless `#warn` is used.

- Binary representation. <-

## Types
i8, u8, i16, u16, i32, u32, i64, u64
i128, u128, f16, f32, f64, f128, sized, unsized,
char, bool, (maybe capital) str, struct, enum,  nil, BigInt, BigFloat, List, Map, Set, Tuple,

`struct` for a structure of data.
`enum` for an Enum type which can also hold data.

// Not sure what to do with this keyword yet
`Tuple`

## [Operators]
`!`: NOT operator.
`&&`: AND operator.
`||` OR operator.

## Keywords
// TODO:
`self`: Refers to current serialized data being looked at

`struct`: for a structure of data.
`enum`: for an Enum type which can also hold data.

## Workspace
- NOT FOR COMPLEXITY, JUST FOR AN ENFORCED CONVENTION. I WANT BINARY

## Actions (Ignore this)
extract env vars
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
    // Should enforce this
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
`const`: Allows the declaration of variables under a constant variable rather than only literals. The type is always inferred to be the lowest possible data type given the context it's used in.

`export`: Allows for the exported value to be used externally when imported.

`import`: Imports `.chrn` file which allows for anything exported within the imported file to be used outside of it.

// TODO

`alias`: Allows for predicates and arguments to be stored within a single function call for convenience.

```chrn
// Maybe if there's only one condition allow no brackets?
alias ShortDefault() = [IsWhitespace]

alias LongDefault(x, y) = [!IsEmpty, Range(x, y), StartsW("ch") EndsW("ern") Contains("chern")]

var->
    special_string: str [LongDefault(0, 5)]
    some_str: str [ShortDefault()]
```

`bind`: Defines where a serialized file is located that should be checked, or deserialized.

## Sections

- Sections instruct how data is parsed. They exist as opposed to keywords so that data is always defined in a readable, predictable manner.

- The `->` operator is used after section keywords to swap to the section. There cannot be more than one of each section.

`var`: Front facing definitions of the data to be serialized or deserialized.

```chrn
// Given struct Person
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
        Ready(str, unsized) // Can optionally store types
        InProgress
        Failed
    }
```

# DOES NOT EXIST YET
`override->`: What to default to when a language doesn't contain a particular type. Language defaults exist but this can change any if needed.

(Probably not a good idea)
There is also a "like" category. A "JAVA_LIKE" category would have all of the int, short, logic for a batch of languages.

`complex->`: Define complex rules

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
            name: str [!IsWhitespace] // Actions would allow for "If WS then Concat("...")"
            color: Color
        }

        enum Color {Red(u8) Blue(u8) Green(u8) } #hex
@end
```


# I FORGOT ABOUT UNICODE
Allows for notation to serialize to be a specific notation. Unicode.

## POSSIBLE FEATURES

(CLI related) Utilities to alter actual main file, such as trimming all strings.

Maybe arithmetic
# Ok maybe

Matrix declarations.
Tensor(N-dim)<f32> more so a convenience wrapper over `List<List<f32>>` (Although tensors are usually in binary) WHICH IS WHY THIS NEEDS A BINARY REPRESENTATION <-----

matrix: Tensor2<f32>

Unified serialization rules for any md file.
Yaml, XML(Forgot this existed), Json, BINARY(I don't know) BINARY, BINARY
