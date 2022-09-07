# Reality Parser 

This is a parser for the `.runmd` format, which is markdown for describing runtime
attributes. Each line of `.runmd` dispatches a message that the parser uses to update its state. Attributes are name/value pairs that also can exist in either a 
stable or transient state. 

Attributes are organized using markdown's code block feature. `.runmd` files can define multiple entities addressed by a name/symbol pair. The parser uses these blocks to initialize an ECS (Entity Component System) World, which is provided by the `specs` library. Each block is assigned an entity, and the block itself can be added to the entity as a component. 

Although this parser is designed to be consumed by the `lifec` runtime, you can think of it as tooling for `specs` itself.

## Example .runmd 
````
``` hello world
+  name         .symbol reality
:: description  .text   This is an example block. 
```
````

The above defines a single entity named "hello world", that contains an 
attribute "name" which has a symbol value of "reality". In addition, 
the attribute has a transient property attribute "description", which 
contains a text value of "This is an example block". Mapping a transient property attribute in this way allows the developer to distinguish between stable
and transient state, which is useful for dynamic programs applications that 
need to eventually stabalize, but have a period where that state must mutate. However, this concern is not within the scope of this parser. This parser is purely
for construction/transportation of attributes. 

In order to to transport/interpret blocks, this library also provides a "wire" protocol, that can encode blocks into 64 byte frames. Frames can then be transported and decoded downstream in order to transmit the state of the World. 

The reason for choosing 64 byte frames is to allow for the block itself to be encoded within metadata of other formats, such as Azure Block Blob ids. 

All of the data created/encoded by this library can be stored in the filesystem, and source control in this format, so that later on the frames can be loaded, and used to re-initialize the World again. 

This allows developers to store and interact with non-source code data in a flexible
way. Since data can be loaded directly into a specs World, components and systems can be added in a modular fashion, using attribute data as config. 
