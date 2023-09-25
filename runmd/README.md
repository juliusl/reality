# Runmd 

Runmd is a language that is embedded in markdown code-blocks, to organize high-level application specific instructions in documentation. 

This crate provides a parser and framework for handling instructions. 

## Background

A typical runmd block looks like,

```md 
```runmd
+ .example                          # This line adds a node
: .property     Hello World         # This line defines a property on that node
<application/example.extension>     # This line loads an extension for the previous node
: .property     Hello Extension     # This line defines a property on the extension once it has loaded
<..other>                           # This line loads another extension by suffix appending to the name of the previously loaded extension
: .property     Hello another Ext   # This line defines a property on the new extension
``` # End block of this block
```

To integrate with the framework, a block provider and node provider are given to the parser. When an instruction to add a node or start a block is encountered, the provider is called and must return a type that implements a `Node` trait.

When an instruction to load an extension or define a property is called, the parser will call the functions on the previously returned node to execute the instruction.

Loading an extension can happen asynchronously and must also return a type that implements the `Node` trait. 

When the entire block has completed parsing, each node will be notified and the parser will proceed to the next block.
