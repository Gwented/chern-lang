// LSP in Go

## Language intent
- This is a scripting language with a markdown language paired with it that allows for typing cross-language serialization configuration. This allows for the avoidance of any annotations or macros that would be required in-line in a language. The scripting language can either use the keyword `bind` to tell it where the serialized file is, or use `@def` and `@end` syntax inside the serialized data itself which allows for the same behavior.

- Features such as conditions, arguments, and anything that is beyond just setting serialized data details or serialized data specific settings are not intended to be heavily used.

# SCRIPT

## BEHAVIOR
- Ends program by default when type information is incorrect unless `#warn` or `#ignore` is used.

- `@def` and `@end` syntax is intended to lock script behavior into one block so that the language constraints can be applied without needing a dedicated outer file that uses `bind`. Everything after `@end` will be considered the serialized file.

- It is not recommended to type above `@def` without comments due to the initial scan needed to make this work needing to avoid reading past `@end` while also skipping comments and quotes that could also have it's keywords inside but are unintended for it to read.

## Types
i8, u8, i16, u16, i32, u32, i64, u64,
i128, u128, f16, f32, f64, f128, sized, unsized,
char, bool, str, struct, enum, nil, BigInt, BigFloat, List, Map, Set, Tuple

`struct`: For defining a structure of data.
`enum`: For defining an enum type which can also hold enumerations with types.
`Tuple`: Holds any amount of types within generic parameters.
`List`: Holds a single generic parameter.
`Set`: Holds a single generic parameter and enforces when checking serialized data that it is in fact a valid set with only one of each value.

`?`: Infers type and expects type consistency throughout entire given serialized data file type.

## Prefix/Unary Operations
`!`: NOT
`-` NEGATE

## Binary Operations
`+` ADD
`-` SUB
`*` MULT
`%` MOD
`&&`: AND
`||` OR

## Workspace
- NOT FOR COMPLEXITY, JUST FOR AN ENFORCED CONVENTION. I WANT BINARY. Make binary

## Actions (Ignore this. Entirely ignore this)
extract env vars

### DOES NOT EXIST YET
`_`: Match all for ignoring parameters

```chrn
alias gopher(x, y) = [!IsEmpty, Range(x = 0.0, y = 5.2), StartsW("ch") EndsW("ern") Contains("chern")]

var->
    special_stir: str [gopher(0.5, _)] // defaults to (0.5, 5.2) 

    stirring: str [gopher(2.0, 5.0)] // Works as normal
```

`e#`: Name bypass for treating a keyword as an identifier.

Example:
```chrn
var->
    x: ~str
nest->
    struct ~str {
        ptr: u8
        len: unsized
        capacity: unsized
    }
```

### DOES NOT EXIST YET
`(range)`: Explicit range syntax. The '=' is required. `0..=5`

## Predicate Keywords
`IsEmpty`: Checks if the given array or string has a length of 0.

`IsWhitespace`: Checks if a string is only whitespace within UTF-8 standards

## Functions

`Equals(Variadic)`: Checks serialized value for equality against given argument


`Range(inclusive, inclusive)`: Checks if the data being viewed matches the range given. For arrays and strings, this checks the length. For numbers, this checks the numeric value.

`Contains(DynType)`: Checks if the data being viewed contains the given literal or numeric.
// Would need to retain notation if this would need to be done
Contains("chern") | Contains(1xF)

`StartsW(DynType)`: Checks if the data being viewed starts with the given literal or numeric.

`EndsW(DynType)`: Checks if the data being viewed ends with the given literal or numeric.

// Does not exist yet
`Regex("0-9a-zA-Z*")`

## Statements

`let`: Allows the declaration of values under a re-usable variable instead of just literals. The type is more so a generic "number", "float" or "string" type as opposed to being something one would define.

`export`: Allows for the exported value to be used externally when imported.
This can be applied to `struct`, `enum`, `let`, and `alias`.

`import`: Imports `.chrn` file which allows for anything exported within the imported file to be used.

`alias`: Allows for predicates and arguments to be stored within a single function call.

```chrn
alias ShortDefault() = [IsWhitespace]

alias LongDefault(x, y) = [!IsEmpty, Range(x, y), StartsW("ch") EndsW("ern") Contains("chern")]

var->
    special_string: str [LongDefault(0, 5)]
    some_str: str [ShortDefault()]
```

`bind`: Defines where a serialized file is located that should be checked, or deserialized. This is not needed if the script file is situated within the serialized data itself.

## Sections

- Sections instruct how data is parsed. They exist so that data is always defined in a readable, predictable manner.

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
        Ready: Tuple<str, unsized>
        InProgress
        Failed
    }
```

# DOES NOT EXIST YET
`override`: What to default to when a language doesn't contain a particular type. Language defaults exist but this can change any if needed.

`complex`: Define complex rules

-------------------------------

## Arguments
`#warn`: Would warn instead of terminating upon seeing a wrongful constraint of any kind.

`#ignore`: Ignores all errors for the type this is applied to for serialized data related errors.

`#scient`, `#hex`, `#bin`, `#octo`: Numeric notations to output in serialized file.

// DOES NOT EXIST
`#unicode`:
`#ignore_rm`: (Would remove anything that didn't align under condition rather than crash or warn.)
//DOES NOT EXIST

- Arguments can be applied to all types within a `struct` or `enum` if put directly after declaration
```
    var->
        name: str #warn
        age: u8
        pets: List<Pet> [!IsEmpty, Range(5, 15)] #warn // This warn only applied to this specific field
    nest->
        // Any failed constraints will be completely ignored
        struct Pet {
            name: str [!IsWhitespace] // (Ignore this) Actions would allow for "If WS then Concat("...")"
            color: Color
        } #ignore

        // Enforces that all types within `Color` will be serialized in hex form
        enum Color {Red: Tuple<u8> Blue: Tuple<u8> Green: Tuple<u8> } #hex
```
```
```

## Other keywords
`as`: Allows for aliasing imports

```
import "definitions.chrn" as defs
import "invalid_utf8_name.chrn" as valid_name

export let VALUE = defs.MAGIC_NUMBER + valid_name.OTHER_MAGICAL_NUMBER
```

#### Full example of language

```chrn
@def
    import "chern.chrn" as cherning
    import "definitions.chrn"

    let stuff = cherning.MAGIC_NUMBER * 2

    var->
        name: str
        age: u8 #warn #bin
        pets: List<Pet> [!IsEmpty, Range(5, 15)]
        opinionated_c: definitions.e#i32
    nest->
        struct Pet {
            name: str [!IsWhitespace]
            color: Color
        }

        enum Color {Red: Tuple<u8> Blue: Tuple<u8> Green: Tuple<u8> } #hex
@end
```

#### Simple example of language

```chrn
bind "serialized_data.chrn"

var-> // #ignore <---- Maybe allow for this to be global
    ptr: ? #ignore
    capacity: ? #ignore
    len: ? #ignore
```

## FORGOT ABOUT UNICODE

## POSSIBLE FEATURES

(CLI related) Utilities to alter actual main file, such as trimming all strings. No.

Matrix declarations.
Tensor(N-dim)<f32> more so a convenience wrapper over `List<List<f32>>` (Although tensors are usually in binary) WHICH IS WHY THIS NEEDS A BINARY REPRESENTATION <-----

matrix: Tensor2<f32>

Unified serialization rules for any md file.
Yaml, XML(Forgot this existed), Json, BINARY(I don't know) BINARY, BINARY, BINARY

# SERIAL
