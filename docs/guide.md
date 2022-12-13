# Reality Programming Guide

## Introduction
Most code bases for large software projects tend to suffer from information chaos. That is, tools and scripts often become undocumented or lost for various reasons. One common experience is that documentation tends to be stored in a seperate location from scripts, meaning both branches start to diverge. This makes the overall architecture for a system to become hidden over time, and makes ramping up a burden. Unless great care is taken at the start of a project to document and organize components, innovating becomes very difficult because changes to the system become increasingly riskier. If we take a step back to understand why we write these scripts/tools, we realize their purpose is to allocate resources that will be used by our systems. So when these scripts and tools become obfuscated by chaos, developers lose their ability to re-create the resources needed to run the system. As an analogy if we applied the scientific method to software development, this is like losing all our control values and procedures in order to re-create our experiments. Ultimately, this leads to re-experimentation to reverse engineer the system and a cycle of solving the same problem again and again.

## Goals

Reality aims to help manage the chaos by laying the foundation that can bridge the domains between documentation and scripting/tools. The goal is to turn components of a system into building blocks, that can be manipulated and rearranged and adapt to a wide-range of different software domains. To accomplish this, 

- First we follow the Entity Component System (ECS) architecture pattern which gives us a good trade-off between performance and flexibility. 
- Then, we introduce a new language called `runmd` which lives within the code block delimitters of `markdown`, and allows us to describe our entities and components. 
- Runmd is used to describe "attributes", which are data structures that store either a single stable value, a single transient value, or both, that lay the foundation of our data model. 
- Attributes are collected and organized into "blocks" which represent individual entities of a system. 
- Finally, blocks can be interpreted at runtime to create components and resources for a system.

Having this foundation in place, we have the ability to describe, in **documentation**, resources and purpose of individual components of our system. This allows us to turn software repositories into "sandboxes", and enables developers to "play" with parts of a system.

## Program structure

Runmd does not aim to replace actual programming languages, rather it attempts to bridge the gap between object-notation and scripting. It is somewhere in-between a functional programming language and pure object notation. 

First, it is important to understand that the evaluation strategy being used is "call-by-name" rather than "call-by-reference". To understand the difference let's compare it to JSON/JavaScript. 

For example, let's say we have some object in JSON, 

```json
{
    "message": "Hello World",
}
```

When evaluated by a JS interpreter, this would create a new object in the runtime, for this example let's put this object in a variable `greeter`. If you were to bring this into OOP terms, you would then apply a "prototype" to this object in order to assign function pointers to it's reference, so that you would be able to write a function such as, 

```js
func greet(obj) {
    console.log(obj.message)
}
```

And be able to assign it to `greeter` so that later in your program you would be able to write, 

```js
greeter.greet()
```

In runmd this same example might look like this, 

```md
<```>
: message .symbol Hello World

+ .greeter
: .greet message
<```>
```

(The angle brackets `<```>` are only to allow this example to be nested within a `.md` file. In a typical `.runmd` file they can be omitted)

Now keeping in mind this is a markdown file, we would probably have documentation with this, so it's possible to write it like this, 

```md
# Greeter
- When interpreted, will print to stdout the control value, "message"
- Uses a stable custom attribute `.greeter` 
  - `.greeter` adds a transient custom attribute `.greet` 
  - `.greet` expects the identity of the control value to print,
    * For example, `: .greet {name of prop}`

<```>
: message .symbol Hello World

+ .greeter
: .greet message
<```>
```

What we have here is documenation that is describing a component called `greeter`, and a transient attribute implemntation it adds called `greet` We also know that when `greet` is evaluated, it will print the value of a property called `message`. 

To interpolate the understanding of this example further, given:

```md
<```>
: enters .symbol Hello World
: exits  .symbol Goodbye World

+ .greeter
: .greet enters
: .greet exits
<```>
```

We can assume from the documentation, that when some system evaluates this block, it should print to stdout, 

```stdout
Hello World
Goodbye World
```

This is what a typical runmd block looks like. It is important to note, that we did not explicitly state **what** is evaluating this block, (just as we didn't state what was evaluating the javascript from before) 

Reality is only focused on parsing and handling the data-model. It expects to be used as a library for an evaluation system, be it a compiler or interpreter. (See `lifec` repo for an actual runtime that uses `runmd`)

# Language Reference

The following a detailed reference of the fundamentals behind runmd. As an outline, it will cover elements, keywords, syntax, and data-model.

## Elements 

### Identities (Idents)
- Idents are specific strings that identifiy each piece of data
- They must match the regex pattern, `[./A-Za-z]+[A-Za-z-._:=/#0-9]*`
- In data-model terms, they are always stored as symbol values

### Comments
- Comments can be placed anywhere on the line and are enclosed with `<text goes here>`
- In addition, a subset of markdown is allowed by the runmd parser and are treated as comments. 
- Each line written this way must be preceeded by one of the following, 
    ```
    #
    *
    -
    //
    ``` md
    ``` runmd
    ```md
    ```runmd
    <
    ```
- If a line begins with any of these characters, the entire line is ignored by the runmd parser.

## Keywords

Runmd uses only two keywords `add` or `define`, and (code) block delimitters (```).

### `add` keyword
- This keyword creates a stable attribute. 
- A stable attribute has only one identity (name) and one value.
- A stable attribute can have transient attributes associated to it, which are referred to as properties, or block properties.
- A stable attribute can be used as a `root` which means that it can be used to define "child" entities.
- When a block is indexed, child entities will be provided along with the root stable attribute.
- Each child entity is allowed to define properties for it self, meaning a single index of a root can provide multiple sets of properties, one for each child and lastly one for the root.

### `define` keyword
- This keyword creates a transient attribute.
- A transient attribute always has two identities, (referred to as name and symbol) and **can** have and use a stable value, but more important is the transient value.
- These transient values are used in conjunction with a stable attribute to create block properties.
- In addition, these values are also used to define control values for blocks.
- Although a root is not required before using `define`, it has the most benefit when used after a stable root. 

### Block delimitters
- Blocks are delimitted by (```) and can be followed by one or more identities. 
- If followed by no identities, this is referred to as the `root` block. The special property of a `root` block, is that it **always** occupies the entity id 0.
- If followed by one identity, this is referred to as a `control block`. A `control block` is special because it can define control values that are inherited by blocks that end with the same identity.
- If followed by two identities, this is referred to as just a `block`. The first identity is referred to as the name, and the second identity is referred to as the control symbol or symbol for short.
- As stated above, if the identity of a control block shares the same identity as the control symbol, any control values define in the control block are inherited by the block.
- This relationship is also used by the parser and interpreting infrastructure to group blocks by control symbols.
- Each block define in runmd has a unique entity id, starting at 0 for the root block.
- These entity id's are not guranteed to be in sequence, as child entities can be defined by any stable attribute defined within a block.

## Syntax

### `add` syntax
- As a shortcut for the `add` keyword, the token `+` can be used instead. For example the following are equivalent. 

```
add name .symbol Ryu

+ name   .symbol Ryu
```

### `define` syntax
- When following an add keyword, the token `:` can be used in place of define, and also implicitly sets the prefix of the name to the name of the root. For example,

```
+ name  .symbol Ryu
: power .int    9001
```

Would give the stable attribute `name` an int property called `power`. The long version of this would be:

```
add name .symbol Ryu
define name power .symbol 9001
```

This will be covered again when going over `custom attributes`, where this syntax feature actually comes into play.

### Attribute syntax

- An attribute is defined mainly by it's type. The following are the core attribute types provided by runmd. These are also referred to as "framing" values.
```
.empty          - Represents an empty value
.bool           - Represents either true or false,
.int            - Represents a signed 32-bit integer
.int_pair       - Represents two signed 32-bit integers
.int_range      - Represents three signed 32-bit integers
.float          - Represents a signed 32-bit float
.float_pair     - Represents two signed 32-bit floats
.float_range    - Represents three signed 32-bit floats
.symbol         - Represents an interned string
.text           - Represents a text buffer stored w/ blob data
.bin            - Represents a binary (u8) vector of bytes
.complex        - Represents a btree-set of interned strings, (also interned)
.reference      - Represents a u64 hash-code of any of the above types
```
- An attribute is always in the form `{dot token}{attribute type name}`
- After these core attribute types, consumers of the library can also extend the parser with "custom attributes"

### Custom Attribute syntax

- A custom attribute must declare a single identifier.

# Appendix 
- Entity Component Systems and Specs
- Azure Block Storage