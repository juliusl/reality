use crate::prelude::Repr;

/// Converts a runir::Repr into a clap::Command,
///
/// Provides a CLI interface utility depending on the available
/// representation levels available.
///
#[cfg(feature = "util-clap")]
impl From<Repr> for clap::Command {
    fn from(value: Repr) -> Self {
        let command_id = value.as_uuid().as_hyphenated().to_string();

        let mut command = clap::Command::new(command_id)
            // Render the markdown docs for the long about
            .long_about(format!("{:#}", value));

        // Node repr allows for configuring various properties
        if let Some(node) = value.as_node() {
            if let Some(annotations) = node.annotations() {
                if let Some(about) = annotations
                    .get("about")
                    .or(annotations.get("help"))
                    .or(annotations.get("description"))
                {
                    command = command.about(about);
                }
            }
        }

        // Recv repr allows for fields defined on the receiver to be configured as arguments
        if let Some(recv) = value.as_recv() {
            if let Some(name) = recv.name() {
                command = command.name(name.to_string());
            }

            if let Some(fields) = recv.fields() {
                for field in fields.iter() {
                    command = command.arg(clap::Arg::from(*field))
                }
            }
        }

        // Host repr allows for extensions defined on the host to be configured as sub-commands
        if let Some(host) = value.as_host() {
            if let Some(exts) = host.extensions() {
                for ext in exts.iter() {
                    command = command.subcommand(clap::Command::from(*ext));
                }
            }
        }

        command
    }
}

impl From<Repr> for clap::Arg {
    fn from(_value: Repr) -> Self {
        todo!()
    }
}
