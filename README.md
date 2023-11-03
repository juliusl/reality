# Reality Project

## Overview

Welcome to our Workspace, an integrated suite comprising `runmd`, `reality`, `loopio`, and `nebudeck`. This collective environment is engineered to offer a comprehensive toolkit for developers looking to harness the power of automation, real-world data, event-driven architectures, and streamlined UX development.

## Components

- **runmd**: A Markdown-based executor that brings your documentation to life by allowing code within Markdown files to be run directly.
- **reality**: A platform designed to connect your applications with real-world data, providing a robust API for data retrieval and manipulation.
- **loopio**: An event-driven framework that acts as the backbone for creating scalable and responsive applications, focusing on non-blocking I/O operations.
- **nebudeck**: A set of `loopio` extensions that enriches the process of building user interfaces, offering tools for both terminal and desktop environments with a focus on plug-in-based architecture.

## Acknowledgements

This project would not be possible without the amazing Rust community and ecosystem. Although an incomplete list, the following libraries and tools were instrumental (in no particular order):

- `tokio` - For a robust Async runtime and concurrency elements.
- `async-trait` - For async functions in traits.
- `logos` - For writing Lexers and Parsers.
- `quote/syn/proc_macro2` - For writing derive macros.
- `cargo/rustup` - For managing toolchains, builds, etc.
- `specs/shred` - Paved the way for this project. These libraries were used extensively in the first incarnation of this project and although they have been decoupled as an optional dependency, it deserves a special place on this list.
-  `imgui/imgui-rs` - For building a really easy to use desktop UI framework.
- `clap` - For building a really easy to use command line parsing framework.
- `tracing` - For a really good logging framework.
- `poem` - For providing an easy to use server framework.
- `hyper` - For providing easy to use networking components.
- `anyhow` - For standard easy to use Error creation.
- `bytes` - For zero-copy byte structures.
- `uuid` - For easy to use uuid parsing and generation.
- `bytemuck` - For easy to use byte casting.
- `serde` - For very good serialization support.
- `OpenAI/ChatGPT` - Translated a stream of consciousness into all of the readme's in the project.

.. and many more, I've forgotten but appreciate no less.

## Contribution

Contributions to the Workspace are welcome. Each sub-project (`runmd`, `reality`, `loopio`, `nebudeck`) has its own contribution guidelines. Please refer to the respective README files for more detailed instructions.

## License

Each tool within the Workspace maintains its own licensing. Please check the individual repositories for their specific licenses.
