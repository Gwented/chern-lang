## Language intent
- This is a scripting language that has a serialized data representation paired with it which allows for typing cross-language serialization configuration. This allows for the avoidance of any annotations or macros that would be required in-line in a language. The scripting language can either use the keyword `bind` to define where the serialized file is, or use `@def` and `@end` syntax inside the serialized data itself which allows for the same behavior.

- Features such as type constraints, type arguments, and anything that is beyond just setting serialized data details or serialized data specific settings are not intended to be heavily used.

- The projected main use-case of this language is as a library for inside of a programming language it is available for, which takes in a path to a script file that could contain the serialized data too, or separately having the script file and data given as arguments.

So something like:

```rust
use chrn;

struct User {
    id: u32,
    age: u8
}

// Uhh wait
fn main() {
    let script_path = "path/to/script/file"
    let user = User { id: 0, age: 0 }

    chrn::serialize(script_path, user)

    // or if serialized data is separate

    let serialized_data = "path/to/serialized/data"
    chrn::serialize(script_path, serialized_data, user)
}
```

# SCRIPT

## BEHAVIOR
- Ends program by default when type information is incorrect unless `#warn` or `#ignore` is used.

- `@def` and `@end` syntax is intended to lock script behavior into one block so that the language constraints can be applied without needing a dedicated outer file that uses `bind`. Everything after `@end` will be considered the serialized file. If the space above the script is not needed, `@end` alone can be used to define a script block (This was unintended behavior but may stay).

- It is not recommended to type above `@def` without comments due to the initial scan needed to make this work being sensitive to accidentally unclosed comments or quotes.

## Types
i8, u8, i16, u16, i32, u32, i64, u64,
i128, u128, f16, f32, f64, f128, sized, unsized,
char, bool, str, struct, enum, nil, BigInt, BigFloat, List, Map, Set, Tuple

`List<T>`: Holds a single generic parameter.
`Set<T>`: Holds a single generic parameter and enforces when checking serialized data that it is in fact a valid set with only one of each value.
`Map<K, V>`: Holds a two generic parameters.
`Tuple<A, B, ..>`: Holds any amount of types within generic parameters.

`struct`: For defining a structure of data.
`enum`: For defining an enum type which can also hold enumerations with types.

`any`: Infers type and expects type consistency throughout entire given serialized data file type.

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
`==` Equal to
`!=` Not Equal to

## Workspace
- NOT FOR COMPLEXITY, JUST FOR AN ENFORCED CONVENTION. I WANT BINARY. Make binary

### DOES NOT EXIST YET
`_`: Match all for ignoring parameters

```chrn
alias gopher(x, y) = [!IsEmpty, Range(x = 0.0, y = 5.2), StartsW("ch") EndsW("ern") Contains("chrn")]

var->
    special_stir: str [gopher(0.5, _)] // defaults to (0.5, 5.2) 

    stirring: str [gopher(2.0, 5.0)] // Works as normal
```

`e#`: Name bypass for treating a keyword as an identifier.

Example:
```chrn
var->
    x: e#let
nest->
    struct e#let {
        ptr: u8
        len: unsized
        capacity: unsized
    }
```

### DOES NOT EXIST YET
// Maybe remove this entirely
`(range)`: Explicit range syntax. The '=' is required. `0..=5`

## Predicate Keywords
`IsEmpty`: Checks if the given array or string has a length of 0.

`IsWhitespace`: Checks if a string is only white-space within UTF-8 standards

## Functions

- Functions cannot be defined beyond the built-in ones provided, but they can be used more extensively within an `alias` statement.

# Does not exist yet

`Equals(Thing)`: Checks serialized value for equality against given argument

`Range(inclusive, inclusive)`: Checks if the data being viewed matches the range given. For arrays and strings, this checks the length. For numbers, this checks the numeric value.

`Contains(DynType)`: Checks if the data being viewed contains the given literal or numeric.
// Would need to retain notation if this would need to be done
Contains("chrn") | Contains(1xF)

`StartsW(DynType)`: Checks if the data being viewed starts with the given literal or numeric.

`EndsW(DynType)`: Checks if the data being viewed ends with the given literal or numeric.

`Regex("0-9a-zA-Z*")`

# Does not exist yet

## Statements

`bind`: Defines where a serialized file is located that should be checked, or deserialized. This is not needed if the script file is situated within the serialized data itself.

`let`: Allows the declaration of values under a re-usable variable if literals are inconvenient. The type is inferred by default.

`export`: Allows for the exported value to be used externally when imported.
This can be applied to `struct`, `enum`, `let`, and `alias`.

`import`: Imports `.chrn` file which allows for anything exported within the imported file to be used.

`alias`: Allows for predicates and arguments to be stored within a single function call.

```chrn
alias ShortDefault() = [IsWhitespace]

alias LongDefault(x, y) = [!IsEmpty, Range(x, y), StartsW("ch") EndsW("ern") Contains("chrn")] #warn

var->
    special_string: str [LongDefault(0, 5)]
    some_str: str [ShortDefault()]
```

## Sections

- Sections instruct how script code is interpreted, similar to how a statement would, but innately. They exist so that data is always defined in a readable, predictable manner.

- The `->` operator is used after section keywords to swap to the section. There cannot be more than one of each section.

- Each section has their own set of allowed other sections to search. Scope searching does not change in any form for searching module imports unless the symbol is explicitly kept private.

`neutral`: This section needs no keyword and exists until a section is explicitly used.

`neutral` allows for:
- importing
- exporting
- Setting bind
- Variable declarations
- Alias declarations

Searchable scopes: `neutral`

```chrn
// Everything above var-> is neutral
import "definitions.chrn"

let fact = 2 + 2 == 3
export alias default() = [IsEmpty]

// Neutral cannot be used after this
var->
    name: str
override->
```


`var`: Front facing definitions of the data to be serialized or deserialized.

`var` allows for:
- Defining serialized data
- Expressing type constraints ([[IsWhitespace, Regex("a-zA-Z")]])
- Using type arguments (#warn/#octal)

Searchable scopes: `neutral` and `nest`

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

`nest` allows for:
- Defining nested data
- Expressing type constraints ([[IsWhitespace, Equals("Hi")]])
- Using type constraint arguments (#warn/#octal)

Searchable scopes: `neutral` and `nest`

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

## Type & Argument constraints

# DOES NOT EXIST YET
`override`: Most important part of the language which controls things such as possible namespace casing to also look for and setting language type defaults. Language defaults exist but this can change any if needed.

`complex`: Define complex rules
Very descriptive!

# DOES NOT EXIST YET
-------------------------------

## Arguments

- Dictates runtime behavior

`#warn`: Would warn instead of terminating upon seeing a wrongful constraint of any kind.

`#ignore`: Ignores all errors for the type this is applied to for serialized data related errors.

`#scient`, `#hex`, `#bin`, `#octal`: Numeric notations to output in serialized file.

# DOES NOT EXIST
`#unicode`:
`#ignore_rm`: (Would remove anything that didn't align under constraint rather than crash or warn.)
# DOES NOT EXIST

- Arguments can be applied to all types within a `struct` or `enum` if put directly after declaration within a nest.

```
    var->
        name: str #warn
        age: u8
        pets: List<Pet> [!IsEmpty, Range(5, 15)] #warn // This warn only applied to this specific field
    nest->
        struct Pet {
            name: str [!IsWhitespace] / (Ignore this)
            color: Color
        } #ignore
        // #ignore is applied to everything in this struct innately

        // Enforces that all types within `Color` will be serialized in hex form
        enum Color {Red: Tuple<u8> Blue: Tuple<u8> Green: Tuple<u8> } #hex
```

## Other keywords
`as`: Allows for aliasing imports

```
import "definitions.chrn" as defs
import "invalid_utf8_name.chrn" as valid_name

export let VALUE = defs.MAGIC_NUMBER + valid_name.OTHER_MAGICAL_NUMBER

var->
    thing: defs.Thingy
```

#### Full example of language

```chrn
@def
    import "chrn.chrn" as cherning
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

var-> // #ignore <---- Maybe allow for this to be global if next to a section
    ptr: any #ignore
    capacity: any #ignore
    len: any #ignore
```

## FORGOT ABOUT UNICODE

## POSSIBLE FEATURES

(CLI related) Utilities to alter actual main file, such as trimming all strings. No.

Matrix declarations.
Tensor(N-dim)<f32> more so a convenience wrapper over `List<List<f32>>` (Although tensors are usually in binary) WHICH IS WHY THIS NEEDS A BINARY REPRESENTATION <-----

matrix: Tensor2<f32>

Unified serialization rules for any md file.
Yaml, XML(Forgot this existed), Json, BINARY(I don't know) BINARY, BINARY, BINARY, BINARY

# SERIAL
