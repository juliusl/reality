# Reality

## Overview

`Reality` is a Rust-based framework designed as the front-end component for the `runmd` language. Its core purpose is to manage resource storage with minimal constraints, enabling seamless integration with a diverse range of types. It upholds the Rust tenets of being `Send + Sync + 'static` for all resources it handles, ensuring thread safety and static lifetimes.

## Key Concepts

- **Resource Storage**: `Reality` maintains a centralized, thread-safe store for resources, ensuring safe and consistent access across different contexts.
- **StorageTarget Trait**: Interactions with `reality` occur through the `StorageTarget` trait, with a provided implementation known as `Shared`.
- **ResourceKey**: Resources are referenced using a `ResourceKey`, which can be optionally specified or generated from the resource's type, supporting a flexible addressing scheme.
- **Type Transmutation**: A `ResourceKey` can be transmuted across different resource types, allowing a single hashed value to be associated with multiple resource types.
- **Block Node Storage**: Within `runmd`, each block node possesses its storage target, while extensions within the same node share a common storage target.

## Resource Scopes

`Reality` incorporates a multilayered resource storage system, providing distinct scopes:

- **Host**: Process-scoped storage accessible by name.
- **Node**: Storage specific to the parent node and its extensions.
- **Transient**: An emphemeral storage context

## Thunk System

`Reality`'s Thunk system facilitates the storage of functions, termed 'Thunks', alongside resources. Thunks are automatically compiled from `runmd` source, each associated with an attribute and provided with a `ThunkContext` containing:

- Line comments from the parsed `runmd` source.
- The unique attribute resource key.
- Initialized resources as defined by `runmd`.
- Access to the three storage scopes mentioned above.

## Derive Macro

`Reality` offers a derive macro to streamline the definition of new types intended for use within the framework. This macro is an integral part of the native plugin model, allowing for extensible project development.

## Getting Started

As `reality` is not a standalone library, the recommended entry point for developers is through `loopio`. The following steps outline a basic setup:

```toml
# Add to your Cargo.toml
[dependencies]
loopio = { git = "https://github.com/juliusl/reality.git", branch = "main" }
```

Begin by exploring the `examples` directory within the `loopio` repository for practical implementation patterns. Use the provided derive macro to create your resource types and integrate them with the Thunk system for full utilization of the framework.

## Contributing & Support

Contributions to the development and enhancement of `Reality` are highly valued:

- **Report Bugs**: If you discover any issues, please [create an issue](https://github.com/juliusl/reality/issues) with detailed information.
- **Suggest Features & Improvements**: Submit a pull request or open an issue to propose new ideas.
- **Get Support**: For questions or assistance, open an issue, and the community or maintainers will offer guidance.

## License

MIT
