# Reality v2 Design Ideas

This document is to capture some design ideas while I work through designing the next update to this library.

# Extension Keyword `<>`

In lifec I allow plugins to configure the attribute parser before it parses the subsequent keywords. This allows plugins to customize configuration making them more flexible and adaptable. The issue with 
this approach was that it was hard to tell which lines were plugins and which were a custom attribute added
by the plugin.

To address this I've decided to add the "extension" keyword `<>`. This keyword was already in use w/in
the wire protocol in order to group frames together for encoding and decoding. So the wiring was already 
there. All that was missing was actual implementation behind it, since .runmd has a completely different
use case. I knew I wanted to incorporate it back into the main language but couldn't figure out how. 

So now I've decided to use it as sort of a marker w/in runmd to allow for declaring sections under a stable
root attribute. For example in lifec there is the `.operation` attribute which allows the user to declare
adhoc sequences of plugin calls w/in the root block.

With this feature the previous behavior is preserved, but also allows for the operation to further define additional sequences that are related to it. For example, consider the current implementations sequence under the extension symbol "call". (This happens to be the default symbol used to index plugins w/ the runtime) This means with reality v2, the following is possible.


```
+  .operation resolve.manifest
<> .println Testing log trace		: .log trace
```

eq to


```
<call>
<> .println Testing log trace 	: .log trace
```

eq to


```
<> call .println Testing log trace	: .log trace
```

eq to


```
: 	.println Testing log trace	: .log trace
```

Now since the point of this is the ability to declare additional sequences under the same root. Let's say we want to add a recovery sequence to this operation. We can then declare it like so,

```
+ .operation resolve.manifest

<recover>
<> .println Recovering resolve.manifest operation		: .log trace

```

eq to 

```
<> recover .println Recovering resolve.manifest operation	: .log trace
```

eq to

```
: recover .println Recovering resolve.manifest operation	: .log trace
```

(Note that the additional custom attribute println add's "log" is written on the same line. This style is 
another way to make plugin custom attributes more readable)

So now when lifec compiles this .runmd file it has a way to create and reference a "recover" sequence.

# Refactoring data indexing

BlockProperties work well enough however after developing lifec_registry for a while, I realized that there was a gap in being able to query data succintly. This leads to a lot of boilerplate around reading data
from BlockProperties, which ultimately makes anything having to do with a thunk context a bit unwieldy. To address this there are a couple of things I wanted to add at the reality level. 

1) A `state` module that allows for easily deserializing state from components stored in world storage.

2) Using a toml document as the primary data index, ultimately replacing block properties all together. 

## State module

I've introduced two main traits to make deserializing state a bit more convienient. Not entirely sure yet how to integrate this with lifec yet but hopefully it all falls in line. The two traits introduced are Provider and Loader. 
This is similar to the SystemProvider pattern, and in general borrows a lot from the System design itself. The main difference is that a System only allows for handling many entities at once, where as the Loader/Provider traits in the
state module focus more on being able to fetch just a single entity, along with any other resources from the world. 

At least in test code the amount of boilerplate is significantly less, so I'm hoping that when integration happens w/ lifec it can end up cleaning a lot of code.


## TOML as the primary data index

Having used toml_edit over the past couple of days, I'm realizing how powerful it is. Also, having worked with lifec_registry for a while, I'm realizing how important it will be to be able to interop with other platforms.
Because data is currently stored in World storage, there really isn't a file format for transient state. .runmd handles stable state, wire format handles network state, so it sort of made sense to address this gap. 

The other issue this addresses is the current way I'm handling the entity index. I wasn't too happy with the solution I ended up using in lifec, but it provided a solution at the time, however I realized soon that I was going
to need a more robust indexing system. TOML and specifically toml_edit seem to really fill this gap. This will allow for more powerful compilation abilities from plugins in lifec providing a more familiar interface when converting parsed runmd into runtime components. I'm hoping eventually it replaces BlockProperties all together so that the Attribute data structure remains in use at the compile / wire layer and not so much up the stack at the runtime layer.

In lifec from the plugin perspective, stable/transient state isn't so important so having references to attributes at that level doesn't have as much use as it does at the lower levels when interpreting runmd.

 
# A Proper .runmd Compiler

Up to this point when writing lifec, I've been able to get a lot of mileage out of the Parser/Interpreter pattern that reality is currently providing. I knew from the beginning that it wasn't exactly the best implementation, but it worked and solved the problem. Now that I've had more experience actually using these components I've realized that a proper "compiler" might make a lot of sense, considering how heavily components are used by the runtime in lifec. The other problem it solves is the ability for plugins to use components at runtime. Currently it is possible for plugins to add components during the "compile" function, however in order to use it at runtime, it requires the plugin to re-compile the workspace in order to generate a new host/world. Although this seems to work well enough, it feels a bit awkward and heavy weight. So my thinking is that a compiler will compile .runmd, and then the actual components and functions can be linked seperately by interpreting the data produced by attributes. So at runtime, a plugin can take the linkable object it cares about, and either read or apply state to it for the entire sequence to use. 

Admittedly it's a bit abstract at the moment, but my gut tells me it's going to provide more flexibility and less boiler-plate higher up the stack.

