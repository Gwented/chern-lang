// Since sections use `->` the idea of NOT tabbing on `->` but instead only tabbing on nests seems like a better formatting heuristic, readability-wise.
Like for:
```chrn
complex->
First {
    second {

    }
}

    First {
        second {

        }
    }
```
// If there is no tab, why pay the indentation overhead of if there were CCurlies?
// The first seems -$#9)$ ok this is the format [ADDRESS ME]

# Language intent
- This is a scripting language that is meant to have a serialized data representation paired with it which allows for typing cross-language serialization configuration. This allows for the avoidance of any annotations or macros that would be required inline in a language, and most favorably allows for cross-language serial configuration. The scripting language can either use the keyword [`bind`](#keywords) to define where the serialized file is, or use `@def` and `@end` syntax inside the serialized data itself which allows for the same behavior.

- Features such as boundaries, directives, and anything that is beyond just setting serialized data details or serialized data specific settings are not intended to be heavily used.

- The projected main use-case of this language is as a library for inside of a programming language it is available for, which takes in a path to a script file that could contain the serialized data too, or separately having the script file and data given as arguments.

So something like:

```rust
use chrn_json;

struct User {
    id: u32
    age: u8
}

// Uhh wait
fn main() {
    let script_path = "path/to/script/file"
    let user = User { id: 0, age: 0 }

    chrn_json::serialize(script_path, user)

    // or if serialized data is separate

    let serialized_data = "path/to/serialized/data"
    chrn_json::serialize(script_path, serialized_data, user)
}
```

Would also have something similar to the pattern of cargo-clippy cargo clippy so that extending upon the tool is easier.

# SCRIPT

## BEHAVIOR
- Ends program by default when type information is incorrect unless [`#warn`](#directives) or [`#ignore`](#directives) is used.

- `@def` and `@end` syntax is intended to lock script behavior into one block so that the language constraints can be applied without needing a dedicated outer file that uses `bind`. Everything after `@end` will be considered the serialized file. If the space above the script is not needed, `@end` alone can be used to define a script block (This was unintended behavior but may stay).

- It is not recommended to type above `@def` without comments due to the initial scan needed to make this work being sensitive to accidentally unclosed comments or quotes.

- The module `core` is a required and implicitly loaded module which defines types, functions, etc. 
Some of what's in `core` like functions and generics are strictly compiler level concepts with no external way of declaration.

- A script region/file can AT MOST be 32KB in size.

Singline comments = //
Multi-line comments = /* */

## Keywords

`bind`: Defines where a serialized file is located that should be checked, or deserialized. This is not needed if the script file is situated within the serialized data itself.

`let`: Allows the declaration of values under a re-usable variable if literals are inconvenient. The type is inferred by default.

`export`: Allows for the exported value to be used externally when imported.
This can be applied to [`struct`](#structural-types), [`enum`](#structural-types), [`let`](#keywords), and [`alias`](#keywords).

`import`: Imports `.chrn` file which allows for anything exported within the imported file to be used.

`alias`: Allows for [predicates](#predicate-keywords) and [directives](#directives) to be stored within a single function call. Aliases much like function calls require that it's parameters have a either a type bound or concrete type given to it (Like i32/Numeric)

`struct`: Declares [structural type](#structural-types)

`enum`: Declares [enumeration type](#structural-types).

`var`, `nest`, `complex`, `override`: Section keywords (more later)

// Maybe putting this first isn't the best idea..
```chrn
// Can apply a condition and directive to itself (More later)
alias ShortDefault() = [IsWhitespace]

alias LongDefault(x: UnsignedInteger, y: UnsignedInteger) = [!IsEmpty, Range(x, y), StartsW("ch") EndsW("ern") Contains("chrn")] #warn

var->
    special_string: str [LongDefault(0, 5)]
    some_str: str [ShortDefault()]
```

`as`: Allows for aliasing imports

```chrn
import "definitions.chrn" as defs
import "invalid_utf8_name.chrn" as valid_name

export let VALUE = defs::MAGIC_NUMBER + valid_name::OTHER_MAGICAL_NUMBER

var->
    thing: defs::Thingy
```

### Other

### DOES NOT EXIST YET
`_`: General purpose identifier from the `core` module


## Types
// Would pointer and function types make sense?

### Basic types

| Type        | Description |
|-------------|-------------|
| `i8`/`u8`    | Signed/unsigned 8-bit integer |
| `i16`/`u16`  | Signed/unsigned 16-bit integer |
| `i32`/`u32`  | Signed/unsigned 32-bit integer |
| `i64`/`u64`  | Signed/unsigned 64-bit integer |
| `i128`/`u128` | Signed/unsigned 128-bit integer |
| `f16`        | 16-bit floating point |
| `f32`        | 32-bit floating point |
| `f64`        | 64-bit floating point |
| `f128`       | 128-bit floating point |
| `sized`      | 4-bit signed pointer-sized integer |
| `unsized`    | 8-bit unsigned pointer-sized integer |
| `char`       | A single Unicode character |
| `bool`       | Boolean value (`true`/`false`) |
| `str`        | UTF-8 encoded string |
| `struct`     | User-defined structure with named fields |
| `enum`       | User-defined enumeration with optional typed variants |
| `nil`        | Generic `nil`/`null` value that adapts to the language it's used in if possible unless specified  |
| `BigInt`     | Integer represented as an unbounded string |
| `BigFloat`   | Floating point type represented as an unbounded string |

### Data structures
- It is **NOT** possible to create generic types outside of what is built-in for data structures. These are intended to be basic translation layers that are typed, which represent how serialized data should be formatted.

`List<T>`: Holds a single generic parameter.
`Set<T>`: Holds a single generic parameter and enforces when checking serialized data that it is in fact a valid set with only one of each value.
`Map<K, V>`: Holds a two generic parameters.
`Tuple<A, B, ..>`: Holds any amount of types within generic parameters.

### struct/enum types
// Would something like C/C++ conversion be fine implicitly converting an enum with types to a union type or would something like #c_union be better?

- Both the structural types [`struct`](#keywords) and [`enum`](#keywords) are ONLY able to be defined within [`nest->`](#sections) sections. The only difference between these two are that structs are required to hold types, but enums can hold either have no type or a type.

Example:
```chrn
nest->
// Commas are optional when defining in `var->` and `nest->`
struct Book {
    title: str,
    chapters: u16
    pages: u16,
    color: Color
}

enum Color {
    Red,
    Blue,
    RGB: Tuple<u8, u8, u8>,
    Hex: str,
}
```


### Boundaries

A `Boundary` is a set of constraints given to a type, same as a trait, interface, concept, etc.

| Boundaries | Bounds | Description |
|---|---|---|
| `SignedInteger` | `SignedInteger` | Signed integer types (`i8`, `i16`, `i32`, `i64`, `i128`) |
| `UnsignedInteger` | `UnsignedInteger` | Unsigned integer types (`u8`, `u16`, `u32`, `u64`, `u128`) |
| `Float` | `Float` | Floating-point types (`f16`, `f32`, `f64`, `f128`) |
| `Bool` | `Bool` | Boolean type |
| `Str` | `Str` | String type |
| `Char` | `Char` | Character type |
| `Runtime` | `Runtime` | Runtime-inferred type |
| `Nil` | `Nil` | nil/null type |
| `Comparable` | `Ordered + CharacterMappable + Bool` | Types that support equality comparison |
| `CharacterMappable` | `Str + Char` | Types that can be mapped as character data |
| `HasLen` | `CharacterMappable + Collection` | Types that have a measurable length |
| `Integer` | `SignedInteger + UnsignedInteger` | Any integer type |
| `Numeric` | `Integer + Float` | Any numeric type |
| `Ranged` | `Numeric + Collection + CharacterMappable` | Types that support range checks |
| `Collection` | `List + Set + Map + Tuple` | Collection types |
| `Ordered` | `Numeric` | Types with a total ordering |

An example of this would be if a parameter expects `Numeric`, it accepts any signed integer, unsigned integer, or float type. If it expects `HasLen`, it accepts strings, characters, and collections.

More often than not this will not actually matter for normal usage since the rules only get complicated when something like `alias` or conditions blocks "ident: type [] <- block" in general are used. Most consumption of this will be a directive sometimes pointing out it's boundaries. Please respect it's boundaries.

## Operators

### Prefix/Unary Operations
`!`: NOT
`-`: NEGATE
`~`: bit NOT

### Binary Operations
`+`: ADD
`-`: SUB
`*`: MULT
`/`: DIV
`%`: MOD
`>`: Greater than
`<`: Less than
`>=`: Greater or equal
`<=`: Less or equal
`==`: Equal to
`!=`: Not Equal to
`&&`: AND
`||`: OR
`&`: bit AND
`|`: bit OR
`^`: bit XOR
`>>`: bit right shift
`<<`: bit left shift

### Pathing
`"::"`: Namespace pathing operator for accessing namespaces like modules, types, and variables

`"."`: Member access operator for accessing fields

### SPECIAL
`=>`: Allows config declarations to do "first=>second=>third{}" instead of "first{second{third{}}}"
to avoid nesting overhead if no properties wish to be set.

## Modules

Syntax for accessing through a module symbol uses [`::`](#pathing) with "module::Type" just like Rust, C++, etc.

### Innate module behavior
Modules in their most basic form are an implicitly found graph with no need for anything but an import call. So, if A imports B, if B imports A, A technically knows C, and so on. This is how `chrn` is intended to be used.

### Workspace :) (NON-EXISTENTENT)
N/A

// Not a keyword, not an operator (maybe), not a function (!), not a predicate
`e#`: Name bypass for treating a keyword as an identifier.

Example:
```chrn

let e#export = 9
var->
    x: e#let
nest->
    struct e#let {
        letness: u8
        timesUsed: unsized
    }
```
### DOES NOT EXIST YET
// Maybe remove this entirely
`(range)`: Explicit range syntax. The '=' is required. `0..=5`

## Functions/Predicates
- Functions/Predicates are a functionality only available within conditions

`IsEmpty`: Checks if the given array or string has a length of 0.

`IsWhitespace`: Checks if a string is only white-space within UTF-8 standards

## Functions

- Functions cannot be defined beyond the built-in ones provided, but they can be used more extensively within an [`alias`](#keywords) statement.

# Does not exist yet

`Equals(Thing)`: Checks serialized value for equality against given argument

`Range(inclusive, exclusive)`: Checks if the data being viewed matches the range given. For arrays and strings, this checks the length. For numbers, this checks the numeric value.

`Contains(DynType)`: Checks if the data being viewed contains the given literal or numeric.
// Would need to retain notation if this would need to be done
Contains("chrn") | Contains(1xF)

`StartsW(DynType)`: Checks if the data being viewed starts with the given literal or numeric.

`EndsW(DynType)`: Checks if the data being viewed ends with the given literal or numeric.

`Regex("0-9a-zA-Z*")`

# Does not exist yet

## Sections

- Sections instruct how script code is interpreted, similar to how a statement would, but innately. They exist so that data is always defined in a readable, predictable manner.

- The `->` operator is used after section keywords to swap to the section. There cannot be more than one of each section.

- Each section has their own set of allowed other sections to search. Scope searching does not change in any form for searching module imports unless the symbol is explicitly kept private. Every section has access to the core language features by default.

- NOTE: All sections below in their "Searchable sections" portion are in order of compiler search priotization. Some sections allow for prefixing with a section name to target one section.

`neutral`: This section has no keyword and exists until a section is explicitly used.

`neutral` allows for:
- [importing](#keywords) & [exporting](#keywords)
- Setting [bind](#keywords)
- Variable declarations
- [`alias`](#keywords) declarations

Searchable sections: `neutral`

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
- Using [conditions](#predicate-keywords) ([[IsWhitespace, Regex("a-zA-Z")]])
- Using [directives](#directives) (#warn/#octal)

Searchable sections: `nest` and `neutral`

```chrn
// Given struct Person
var->
    name: str
    age: u8

// But given nested data such as
    account: Account
// it would need a nest section
```



`nest`: Allows for the definition of a struct or enum

`nest` allows for:
- Defining nested data
- Expressing [type boundaries](#predicate-keywords) ([[IsWhitespace, Equals("Hi")]])
- Using [directives](#directives) (#warn/#octal)

Searchable sections: `var`, `nest` and `neutral`

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

// Maybe rename to attributes or properties
`complex`: Define complex rules associated with an already defined type. This is where settings attributes like what casing to look for or default values to assign would be set.

Searchable sections: `var`, `nest` and `neutral`

`complex` can have at most two nesting levels. Which is "Thing { inner {} }" where "Thing { inner { inner_inner {} } }" would be an error at inner_inner.

To avoid redundancy the examples will use the following structures:

```chrn
nest->
    struct Cat {
        name: str
        age: u8
        mortgage: BigFloat
        stressLevel: StressLevel
    }

    enum StressLevel {
        HIGH
        MEDIUM
        LOW
    }

    struct Home {
        price: BigFloat
        pastOwners: u16
    }
```


### Options Assignments
// Should explain all built-in schemas and options associated
- Option assignments are built-in options associated with schemas, which align with what type is currently being used. (Field, struct, etc.)

```chrn
complex->
    // This is implicitly known as the already defined "Cat" struct within a complex section
    StressLevel {
        // This `casing` option would check if "snake_case" is a valid preset option, then attempt to 
        // string match multiple versions of the same name and procedurely convert it's convention.
        // ie. "StressLevel" would be searched by "StressLevel", "stress_level" and "Stress_Level"
        casing = ["snake_case", "UpperSnakeCase"]

        // If only one value is present the outer brackets can be omitted
        identifiers = "Happy"
    }
```

### Config members
- Configuration also allows for nesting in the case of defining special properties specific to members. This attempts to find the member with the identifier given.

```chrn
complex->
    Cat {
        cases = ["snake_case", "UpperSnakeCase"]

        // Looks for member with identifier "stressLevel"
        stressLevel {
            // Looks for member "HIGH" within the type of stressLevel which is the most recent parent
            HIGH {
                idents = "High"
            }
        }

        identifiers = "Happy"

        mortgage {/*code*/}
    }
```

// SHOW OVER-NESTING EXAMPLE HERE

### Schemas
// Should address that the initial naming given to identifiers like in "struct Person {name:str}" is just the identifier the langauge wants as a stable indicator for that particular field, and is alterable easily through the complex cfgs.
// Need some migration specific tools that help with fixing old versions instead of "idents" possibly being the only form of mitigation

## CONCEPTS HANDLED BY CONDITIONS BUT IF THEY WERE REMOVED USED THESE OOPTIONS
if_nil_ignore:
deserialize_nil_as_default(`bool`)
## CONCEPTS HANDLED BY CONDITIONS BUT IF THEY WERE REMOVED USED THESE OOPTIONS

NOT DONE YET NOR FINAL
struct, enum, member schemas

struct/enum options:
idents(`str`): Allows for multiple identifiers to be matched (For the type itself)
// Should support all the names like kebab_case, snake_case, etc. (Kebab????)
cases: Allows for different cases to be matched. Is more so a convenience over idents.
serialized_ident: The given identifier is what the serialized data is output as. By default, this value is just the field's name. Meaning, for "First { second {} }" second is what the serialized data is output as by default unless specified.
serialized_output_ordering: Orders the members of this type in the specified manner. (Would probably look like "serialized_output_ordering(`str`) = ["field1", "field2"]" where if there is a `field3` it just puts it just greedily orders)
// Maybe this should be an internal macro, even though that means we lost the config outside of code part.
skip_unknown_members(`bool`): If a member in this type was found that does not exist in the chrn defined file then it's not an error it's just skipped. (This is maybe `false` by default and errs)

member options:
default_val: If the language has a null/None concept, this is applied. (if possible)
idents: Allows for multiple identifiers to be matched for the given member (Not its type)
// Should support all the names like kebab_case, snake_case, etc.
cases: Allows for different cases to be matched. Is more so a convenience over idents.
(as/under/ident)
serialized_ident: The given identifier is what the serialized data is output as. By default, this value is just the field's name. Meaning, for "First { second {} }" second is what the serialized data is output as by default unless specified.

NOTE: What if there was a way to give specified values to the options? Like, "serialized_output_ordering = #alphabetical"? Would probably need a better way internally to do this since this could get bad quickly.

NOT DONE YET NOR FINAL

### Searching var/nest scopes specifically

The section keyword `var` or `nest` can be used before a config root to narrow the search range, and most notably remove same identifier conflicts.

```chrn
var->
    same: i32
nest->
    struct same {}
complex->
    // Avoids any form of conflict
    var same {}
    nest same {}
```

#### IMPORTANT NOTES

##### Syntax shortening with `=>`
`=>` can be used to shorten syntax if no properties are desired.
Example:
```chrn
// What if the docs refuse to compile?
// I think that means it's wrong
nest->
    struct First {second: Second}
    struct Second {val: i32}
complex->
    // Without arrows:
    First { second { idents = "different" } }

    // With arrows:
    First=>second { idents = "different" }
```

NOTE: This is mainly meant for `override` since it has no nesting limit and may prefer such arrow usage.

# UPDATE
##### Member access
Member access like:
```chrn
    other_namespace::Thing {}
```
**CANNOT** be done.

This is because there does not currently seem to be any intrinsic benefit to doing so other than increasing the possible complexity of code, nor is it clear how this would affect actual property setting.

For example, It could be:
* Implicitly globally applied to all usages of Thing
* Directly mutating the external module that declared Thing so that it uses it's specific config
* An optional external mutation where you use a directive to determine whether or not it should mutate
None of these are very concrete to where just enforcing that the current module's defined type must be the root for any config used is currently the only way this is done (This is not final)

##### Recursive types within configs
Recursive types are allowed to be defined within configs.

For example:
```chrn
nest->
struct Orange {
    orange: Orange
}
complex->
Orange {
    // This applies to the actual type of `Orange` and it's possible identifiers
    idents = ["Urang", "Crust"]
    orange {
        idents = "Recursive Orange"
        // This applies to the member's possible identifiers
    }
}
```

# DOES NOT EXIST YET
`override`: Controls elements such as possible namespace casing to also look for and setting language specific defaults. Language defaults exist intrinsically but this can change any if needed.

override sections use the same exact semantics as the earlier explained `complex` scope's configs. The only difference is that override does NOT restrict nesting levels, and strictly acts on language known semantics.

# DOES NOT EXIST YET
-------------------------------

## Directives

- Directives change how the compiler interprets certain code. This can range from changing how serialized data is represented to changing default runtime behavior.

`#warn`: Would warn instead of terminating upon seeing a wrongful constraint of any kind.

`#ignore`: Ignores all errors for the what this is applied to for serialized data related errors.

`#scient`, `#hex`, `#bin`, `#octal`: Numeric notations to output in serialized file instead of base ten.

`#unicode`: Output characters as their unicode such as '\u{1F480}' instead of '💀'

# DOES NOT EXIST
`#ignore_rm`: (Would remove anything that didn't align under constraint rather than crash or warn.)
// Maybe collapse some conditions into serialization options. Seems easier to do, "skip_if = self == 30" than timeout: u64 [Equals(30)] #skip_if or [Equals(30) #skip_if, ] Ok maybe this doesn't look that bad.
// What if we threw the concept of conditions INTO options?
`#skip_if`: Um
# DOES NOT EXIST

- Directives can be applied to all types within a `struct` or `enum` if put directly after declaration within a nest.

```
    var->
        name: str #warn
        age: u8
        // #warn is only applied to this particular field.
        pets: List<Pet> #warn
    nest->
        struct Pet {
            name: str
            color: Color
        } #ignore
        // #ignore is applied to everything in this struct innately

        // Enforces that all types within `Color` will be serialized in hex form
        enum Color {Red: Tuple<u8> Blue: Tuple<u8> Green: Tuple<u8> } #hex
```

Important examples:
```chrn
var->
    // This is fine because #bin expects numeric
    num: i32 #bin

    // Although all of Point satisfies numeric, this is an error because it's not clear what this should
    // apply to. Should this apply to Point, but only for this specific field instance? (Wait should it?)
    // Adding this maybe

    point: Point #bin

    // This is fine because neither of these have type boundaries
    other_point: Point #warn #ignore
nest->
    struct Point {
        x: i64
        y: i64
    // This works fine because it's declaring this at the struct level
    } #bin
```

## Conditions
Conditions enforce that a type must adhere to it's conditions given or else the program will fail unless a directive is used to specifically change the default behavior behavior.

The general purpose of this functionality is to allow for enforcing safety at not just types, but also an input by input basis.

Directives can even further enhance conditions where (NOT DONE YET BUT WOULD REFER TO #ignore_rm)

```chrn
alias gopher(x: Numeric, y: Numeric) =
    [!IsEmpty, Range(x = 0, y = 5), StartsW("ch") EndsW("ern") Contains("chrn")] #warn

var->
    special_stir: str [gopher(2, _)] // defaults to (2, 5)

    stirring: str [gopher(3, 15)] // Works as normal
```

#### Full example of language

```chrn
@def
    import "chrn.chrn" as cherning
    import "definitions.chrn"

    let stuff = cherning::MAGIC_NUMBER * 2

    var->
        name: str
        age: u8 #warn #bin
        pets: List<Pet> [!IsEmpty, Range(5, 15)]
        opinionated_c: core::i32
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

var->
    ptr: Runtime #ignore
    capacity: Runtime #ignore
    len: Runtime #ignore
```




## POSSIBLE FEATURES

(CLI related) Utilities to alter actual main file, such as trimming all strings. No.

Matrix declarations.
Tensor(N-dim)<f32> more so a convenience wrapper over `List<List<f32>>` (Although tensors are usually in binary) WHICH IS WHY THIS NEEDS A BINARY REPRESENTATION <-----

matrix: Tensor2<f32>

Unified serialization rules for any md file.
Yaml, XML(Forgot this existed), Json, BINARY(I don't know) BINARY, BINARY, BINARY, BINARY

# SERIAL
