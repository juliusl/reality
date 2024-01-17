use std::fmt::Debug;

use clap::Arg;
use runir::prelude::*;
use tracing::{debug, trace, warn};

use super::Program;
use crate::ResourceKey;
use crate::Workspace;

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

    /// Returns an iterator w/ mutable access to each program contained in the package,
    ///
    pub fn programs_mut(&mut self) -> impl Iterator<Item = &mut Program> {
        self.programs.iter_mut()
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
        // Package name is the name of the command
        let mut command = clap::Command::new(name);

        // Map out all of the programs into their own subcommand
        for m in value.search("*") {
            let add = m.host.address().expect("should be an address");
            debug!("Adding subcommand {:?}", add);
            let mut group = clap::Command::new(add.to_string());

            // Check if package should be skipped
            if m.node
                .and_then(|n| n.annotations())
                .map(|a| matches!(a.get("internal"), Some(val) if val.as_str() == "true"))
                .unwrap_or_default()
            {
                trace!("skipping internal");
                continue;
            }

            // Add argument settings from annotations
            if let Some(annotation) = m.node.and_then(|n| n.annotations()) {
                for (k, v) in annotation.iter().filter(|(k, _)| k.starts_with("arg.")) {
                    let name = k.trim_start_matches("arg.").to_string();
                    let mut arg = Arg::new(&name).default_value(v);
                    if name.len() == 1 {
                        arg = arg.short(name.chars().next().expect("should exist"));
                    } else {
                        arg = arg.long(name);
                    }
                    group = group.arg(arg);
                }
            }

            // Add any node extensions as a sub command
            if let Some(ext) = m.host.extensions() {
                if let Some(about) = resolve_help_about(m.node) {
                    group = group.about(about);
                }

                group = ext.iter().fold(group, |group, e| {
                    if let Some(ext) = create_ext_command(group.clone(), &add, e) {
                        ext
                    } else {
                        group
                    }
                });

                command = command.subcommand(group);
            }
        }

        command
    }
}

/// Resolve helo/about string,
///
fn resolve_help_about(node: Option<NodeRepr>) -> Option<String> {
    node.and_then(|n| n.doc_headers())
        .and_then(|d| d.first().cloned())
        .or(node
            .and_then(|n| n.annotations())
            .and_then(|a| a.get("help").cloned()))
}

/// Create ext command,
///
fn create_ext_command(group: clap::Command, host: &str, e: &Repr) -> Option<clap::Command> {
    // Resolve help description for command group
    let help = resolve_help_about(e.as_node());

    if let Some(addr) = e.as_host().and_then(|h| h.address()) {
        let fragments = addr.split('/').collect::<Vec<_>>();
        trace!("Adding ext as subcommand {:?}", fragments);

        if fragments.len() > 3 {
            warn!("Cannot add as subcommand, more than 3 fragments");
            return None;
        }

        match fragments[..] {
            [g, command, internal_ext, ..] if g == host => {
                trace!("Adding ext as subcommand {g} {command} {internal_ext}");
                // This should be unused from cli, but is used to store the ext type name
                let ext_arg = Arg::new("internal_ext")
                    .long("internal_ext")
                    .hide_short_help(true)
                    .hide_long_help(true)
                    .default_value(internal_ext.to_string());

                let mut sub = clap::Command::new(command.to_string()).arg(ext_arg);
                if let Some(help) = help {
                    sub = sub.about(help);
                }

                // Add any annotations starting w/ arg.* as arguments
                if let Some(node) = e.as_node() {
                    if let Some(annotation) = node.annotations() {
                        for (k, v) in annotation.iter().filter(|(k, _)| k.starts_with("arg.")) {
                            let name = k.trim_start_matches("arg.").to_string();
                            let mut arg = Arg::new(&name).default_value(v);
                            if name.len() == 1 {
                                arg = arg.short(name.chars().next().expect("should exist"));
                            } else {
                                arg = arg.long(name);
                            }
                            sub = sub.arg(arg);
                        }
                    }
                }

                // Add any fields that have ffi enabled as arguments
                if let Some(recv) = e.as_recv() {
                    trace!("Ext is recv, checking fields");
                    if let Some(fields) = recv.fields() {
                        let args = fields.iter().fold(vec![], |mut args, f| {
                            trace!("Trying to add field as arg\n\n{:#}\n", f);
                            if let Some((field_name, field_help, _, value_parser)) =
                                f.split_for_arg()
                            {
                                trace!("Adding field `{field_name}` as arg");
                                let mut arg = Arg::new(field_name)
                                    .long(field_name)
                                    .value_parser(value_parser);

                                // Include an empty field packet
                                if let Some(packet) = ResourceKey::with_repr(*f)
                                    .field_packet()
                                    .and_then(|p| bincode::serialize(&p).ok())
                                {
                                    trace!("Adding base64 encoded empty packet as arg");
                                    let arg_name =
                                        format!("internal_{field_name}_field_packet_enc");
                                    let arg = Arg::new(&arg_name)
                                        .long(arg_name)
                                        .hide_short_help(true)
                                        .hide_long_help(true)
                                        .help("base64 encoded empty field packet")
                                        .default_value(base64::encode(packet));
                                    args.push(arg);
                                }

                                // Get the node input and set as the default value
                                if let Some(input) = f.as_node().and_then(|n| n.input()) {
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
                            args
                        });

                        if !args.is_empty() {
                            sub = sub.args(args);
                        }
                    }
                }

                return Some(group.subcommand(sub));
            }
            _ => {
                warn!("Unimplemented command {:?}", fragments);
                // unimplemented!()
            }
        }
    }

    None
}
