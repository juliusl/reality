# \`\`\`runmd

## Overview

`runmd` is a Rust-based language that elevates the markdown experience by embedding executable blocks within markdown files. The language structure is composed of blocks, nodes, and instructions, forming a shallow tree with blocks serving as roots. These roots branch out into nodes, and each node can bear multiple leaves termed as "extensions."

## Key Concepts

- **Blocks**: The foundational structure of a `runmd` program, starting with a markdown code block annotated with `runmd`.
- **Nodes**: Represent separate branches within a block and are initiated with a '+' symbol. Nodes contain instructions and can have properties and extensions.
- **Instructions**: Each line within a node is parsed into an instruction, which is the actionable element of the language.
- **Extensions**: Leaves of a node, defined using a media type format, which specify the output or the operational context of a node.

## Writing runmd

To write a `runmd` script, one starts with a code block labeled `runmd` and proceeds to configure nodes with instructions, properties, and extensions as needed:

```md
```runmd
+ .node_name input # comment
: .property_name value # property comment
<application/json> extension input # extension comment

```

**Basic Syntax:**

- **Start a block**: ` ```runmd`
- **Add a node**: `+ .node`
- **Add a property**: `: .property`
- **Add an extension**: `<extension_name>`
- **End a block**: ` ``` `

Special considerations include escaping '#' with double quotes and the distinction that extensions do not receive a tag value.

## Installation & Prerequisites

`runmd` is primarily a backend parser and lexer for developers interested in integrating runnable markdown into their applications. To get started, clone the repository and refer to the language guide provided.

## Getting Started

Dive into the `tests` directory for a minimum viable implementation. This will demonstrate the syntax and functionality of `runmd`, providing a hands-on introduction to the language.

## Architecture

When parsing `runmd` blocks, the library emits a flat list of instructions which then drive the parser. Implementing `runmd` involves setting up block and node providers that handle the beginning of blocks and addition of new nodes, respectively. Both providers must yield a type that implements the Node trait, which includes the ExtensionLoader trait. To streamline this process, a `BoxedNode` type is utilized.

## Contributing

Contributions to `runmd` are highly encouraged:

- **Bugs**: Report bugs by [creating an issue](https://github.com/juliusl/reality/issues).
- **Features**: Suggest new features with a pull request or an issue.
- **Support**: For support, create an issue or reach out via our community channels.

## License

MIT
