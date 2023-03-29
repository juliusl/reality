# Example Usage - Plugins

In this example we'll design a plugin system within runmd.

**Example runmd definition**
```runmd
+ .plugin                               # Extensions that can be used when defining a plugin
<> .path                                # Indicates that the variable should be a path
: canonical     .bool                   # If enabled, will check if the value is a canonical path
: cache         .bool                   # If enabled, indicates that the file at path should be read
<> .map                                 # Indicates that the variable will have key-value pairs within the root
<> .list                                # Indicates that the variable can be a list of values

+ .plugin process                       # Plugin that executes a child process
: cache_output 	.bool 	                # Caches output from process to a property
: silent		.bool 	                # Silences stdout/stderror from process to parent
: inherit		.bool	                # Inherits any arg/env values from parent's properties
: redirect		.symbol                 # Redirects output from process to path
: cd			.symbol	                # Sets the current directory of the process to path
: env			.symbol	                # Map of environment variables to set before starting the process
: arg			.symbol	                # List of arguments to pass to the process
: flag		    .symbol	                # List of flags to pass to the process

<path>  .redirect : canonical .true     # Should be a canonical path
<path>  .cd                             # Should be a path
<map>   .env                            # Should be a map
<list>  .arg                            # Should be a list
<list>  .flag                           # Should be a list
```

If we compile the above config documentation will be compiled as well. We will dissect the above using each piece of documentation compiled.

---

```
Found doc -- 
        .plugin
        Add
        With("plugin", Symbol(""))
        Doc("# Extensions that can be used when defining a plugin")

```

This indicates that we are adding a new root called `plugin`. Within this root we can continue to define extensions that are related to this root.

--- 

```
Found doc -- 
        .plugin.path
        Extension
        Doc("# Indicates that the variable should be a path")
        With("canonical", Bool(false))
        Doc("# If enabled, will check if the value is a canonical path")
        With("cache", Bool(false))
        Doc("# If enabled, indicates that the file at path should be read")

Found doc -- 
        .plugin.map
        Extension
        Doc("# Indicates that the variable will have key-value pairs within the root")

Found doc -- 
        .plugin.list
        Extension
        Doc("# Indicates that the variable can be a list of values")
```

Here we've declared 3 extensions related to our root, 

- **path**: This extension will ensure that a property can be converted into an OS file path
- **map** : Indicates that a property is using the "map" pattern
- **list**: Indicates that a property will be a list of properties

Extensions can be used to enrich config data and can be used by tooling as hints on how to apply/load data.

Next, we will create a new root based on `plugin` called `process`. 

---

```
Found doc -- 
        .plugin.process
        Add
        With("plugin", Symbol("process"))
        Doc("# Plugin that executes a child process")
```

As a root, we can define properties specific to this type of plugin, as well as add extensions provided by plugin to annotate specific properties within this root.

--- 

```
Found doc -- 
        .plugin.process
        Add
        With("plugin", Symbol("process"))
        Doc("# Plugin that executes a child process")
Found doc -- 
        .plugin.process
        Define
        With("cache_output", Bool(false))
        Doc("# Caches output from process to a property")
Found doc --
        .plugin.process
        Define
        With("silent", Bool(false))
        Doc("# Silences stdout/stderror from process to parent")
Found doc --
        .plugin.process
        Define
        With("inherit", Bool(false))
        Doc("# Inherits any arg/env values from parent's properties")
Found doc --
        .plugin.process
        Define
        With("redirect", Symbol(""))
        Doc("# Redirects output from process to path")
Found doc --
        .plugin.process
        Define
        With("cd", Symbol(""))
        Doc("# Sets the current directory of the process to path")
Found doc --
        .plugin.process
        Define
        With("env", Symbol(""))
        Doc("# Map of environment variables to set before starting the process")
Found doc --
        .plugin.process
        Define
        With("arg", Symbol(""))
        Doc("# List of arguments to pass to the process")
Found doc --
        .plugin.process
        Define
        With("flag", Symbol(""))
        Doc("# List of flags to pass to the process")
```

These are the properties specific to this type of plugin root. In addition here are the extensions being applied w/ this root.

--- 

```
Found doc --
        .plugin.process.path.redirect
        Extension
        With("canonical", Bool(true))
        Doc("# Should be a canonical path")
Found doc --
        .plugin.process.path.cd
        Extension
        Doc("# Should be a path")
Found doc --
        .plugin.process.map.env
        Extension
        Doc("# Should be a map")
Found doc --
        .plugin.process.list.arg
        Extension
        Doc("# Should be a list")
Found doc --
        .plugin.process.list.flag
        Extension
        Doc("# Should be a list")
```

Note ".plugin.process.path.redirect" has the canonical property enabled. When the above runmd is compiled into a toml document, it will look like the below.

---

```toml
[properties."plugin.path"]
cache = false
canonical = false

[properties."plugin.process"]
arg = ""
cache_output = false
cd = ""
env = ""
flag = ""
inherit = false
redirect = ""
silent = false

[properties."plugin.process.path.redirect"]
canonical = true

[block.""]
roots = [
	"plugin", 
	"plugin.process"
]

[root.plugin]
extensions = [
	"plugin.path", 
	"plugin.map", 
	"plugin.list"
]

[root."plugin.process"]
extensions = [
	"plugin.process.path.redirect", 
	"plugin.process.path.cd", 
	"plugin.process.map.env", 
	"plugin.process.list.arg", 
	"plugin.process.list.flag"
]
```

By using this document when we parse config in our application we have alot of relevant information to use when tooling our application. Note how the extension above is exported,

```toml
[properties."plugin.process.path.redirect"]
canonical = true
```
---

So now when this is used in an actual application, we can know how we should handle the input values. 

For example,

```runmd app
+ .runtime
<plugin> 	.process    cargo test
: RUST_LOG 	.env        reality=trace
:           .arg	    --package
:           .arg        reality
:           .redirect   .test/test.output
```

In this example we're making use of the process plugin as an extension and configuring the properties we need. Our tooling has the settings provided by the previous definition, and can use that information to validate the properties we've set in this usage.


When this block is compiled the toml will look like this,

```toml
[properties."app.#block#.runtime.plugin.process.cargo test"]
RUST_LOG = "reality=trace"
arg = ["--package", "reality"]
env = "RUST_LOG"
redirect = ".test/test.output"

[block."app.#block#"]
roots = ["app.#block#.runtime"]

[root."app.#block#.runtime"]
extensions = ["app.#block#.runtime.plugin.process.cargo test"]
```

Now if we were to write tooling for this, once we've imported this toml we can then query for the plugin process with the pattern "plugin.process.(program)", which will end up returning these properties:

```toml
[properties."app.#block#.runtime.plugin.process.cargo test"]
RUST_LOG = "reality=trace"
arg = ["--package", "reality"]
env = "RUST_LOG"
redirect = ".test/test.output"
```

And since we see the redirect property, and we're making our tooling aware that this property has an extension, we can then validate the property.
