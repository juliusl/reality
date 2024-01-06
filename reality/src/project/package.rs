use std::fmt::Debug;

use clap::Arg;
use runir::prelude::*;
use tracing::{trace, warn, debug};

use crate::{ResourceKey, Workspace};

use super::Program;

/// Package containing all programs compiled from a project,
///
#[derive(Clone)]
pub struct Package {
    /// Workspace this package was derived from,
    ///
    pub(crate) workspace: Workspace,
    /// Programs,
    ///
    pub(crate) programs: Vec<Program>,
}

impl Package {
    /// Searches for a program w/ name,
    ///
    /// **Note** If `*` is used all programs w/ addresses are returned.
    ///
    pub fn search(&self, name: impl AsRef<str>) -> Vec<ProgramMatch> {
        let mut matches = vec![];
        for p in self.programs.iter() {
            if p.context()
                .ok()
                .and_then(|a| a.attribute.host())
                .and_then(|a| a.address())
                .filter(|p| p.ends_with(name.as_ref()) || name.as_ref() == "*")
                .is_some()
            {
                if let Some(host) = p.context().ok().and_then(|a| a.attribute.host()) {
                    matches.push(ProgramMatch {
                        host,
                        node: p.context().ok().and_then(|a| a.attribute.node()),
                        program: p.clone(),
                    });
                }
            }

            matches.extend(p.search(name.as_ref()));
        }
        matches
    }
}

/// Struct containing the result of a program search,
///
pub struct ProgramMatch {
    /// Host representation
    ///
    pub host: HostRepr,
    /// Node representation
    ///
    pub node: Option<NodeRepr>,
    /// Matched program
    ///
    pub program: Program,
}

impl Debug for ProgramMatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramMatch")
            .field("host", &self.host)
            .field("node", &self.node)
            .finish()
    }
}

impl From<Package> for clap::Command {
    fn from(value: Package) -> Self {
        let name = &value.workspace.name;
        let mut command = clap::Command::new(name);

        fn resolve_help_about(node: Option<NodeRepr>) -> Option<String> {
            node.and_then(|n| n.doc_headers())
                .and_then(|d| d.first().cloned())
                .or(node
                    .and_then(|n| n.annotations())
                    .and_then(|a| a.get("help").cloned()))
        }

        for m in value.search("*") {
            if m.node
                .and_then(|n| n.annotations())
                .map(|a| matches!(a.get("internal"), Some(ref val) if val.as_str() == "true"))
                .unwrap_or_default()
            {
                trace!("skipping internal");
                continue;
            }

            if let Some(ext) = m.host.extensions() {
                let add = m.host.address().expect("should be an address");
                debug!("Adding subcommand {:?}", add);

                let mut group = clap::Command::new(add.to_string());

                if let Some(about) = resolve_help_about(m.node) {
                    group = group.about(about);
                }

                for e in ext.iter() {
                    // Resolve help description for command group
                    let help = resolve_help_about(e.as_node());

                    if let Some(addr) = e.as_host().and_then(|h| h.address()) {
                        let fragments = addr.split('/').collect::<Vec<_>>();
                        trace!("Adding ext as subcommand {:?}", fragments);

                        if fragments.len() > 3 {
                            // TODO: Join the middle w/ underscores
                            warn!("Cannot add as subcommand, more than 3 fragments");
                            continue;
                        }

                        match fragments[..] {
                            [g, command, _ext, ..] if g == add.as_str() => {
                                trace!("Adding ext as subcommand {g} {command} {_ext}");
                                // This should be unused from cli, but is used to store the ext type name
                                let ext_arg = Arg::new("_ext")
                                    .long("_ext")
                                    .default_value(_ext.to_string());

                                let mut sub = clap::Command::new(command.to_string()).arg(ext_arg);
                                if let Some(help) = help {
                                    sub = sub.about(help);
                                }

                                let mut args = vec![];
                                if let Some(recv) = e.as_recv() {
                                    trace!("Ext is recv, checking fields");
                                    if let Some(fields) = recv.fields() {
                                        for f in fields.iter() {
                                            trace!("Trying to add field as arg\n\n{:#}\n", f);
                                            if let Some((field_name, field_help, _, value_parser)) =
                                                f.split_for_arg()
                                            {
                                                trace!("Adding field `{field_name}` as arg");
                                                let mut arg = Arg::new(field_name)
                                                    .long(field_name)
                                                    .value_parser(value_parser);

                                                // Include an empty field packet
                                                if let Some(field_packet) =
                                                    ResourceKey::with_repr(*f).field_packet()
                                                {
                                                    if let Ok(packet) =
                                                        bincode::serialize(&field_packet)
                                                    {
                                                        trace!("Adding base64 encoded empty packet as arg");
                                                        let arg_name = format!(
                                                            "_{field_name}_field_packet_enc"
                                                        );
                                                        let field_packet = Arg::new(&arg_name)
                                                            .long(arg_name)
                                                            .help(
                                                                "base64 encoded empty field packet",
                                                            )
                                                            .default_value(base64::encode(packet));

                                                        args.push(field_packet);
                                                    }
                                                }

                                                // Get the node input and set as the default value
                                                if let Some(input) =
                                                    f.as_node().and_then(|n| n.input())
                                                {
                                                    arg = arg.default_value(input.to_string());
                                                }

                                                // Set the field_help
                                                if let Some(help) = field_help {
                                                    arg = arg.help(help);
                                                }

                                                args.push(arg);
                                            } else {
                                                trace!("did not split for arg");
                                            }
                                        }
                                    }
                                }

                                if !args.is_empty() {
                                    sub = sub.args(args);
                                }

                                group = group.subcommand(sub);
                            }
                            _ => {
                                warn!("Unimplemented command {:?}", fragments);
                                // TODO: Join the middle w/ underscores
                                // unimplemented!()
                            }
                        }
                    }
                }

                command = command.subcommand(group);
            }
        }

        command
    }
}
