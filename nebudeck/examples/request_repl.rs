// use loopio::engine::Engine;
// use loopio::host;
// use loopio::prelude::Workspace;
// use nebudeck::ControlBus;
// use nebudeck::terminal::TerminalApp;
// use nebudeck::terminal::Terminal;
// /// Minimal example for starting a new terminal repl interaction,
// /// 
// #[tokio::main]
// async fn main() {
//     let mut workspace = Workspace::new();
//     workspace.add_buffer(
//         "request_repl.md", 
//         r#"
//         ```runmd
//         + .operation            get
//         <ux/nebudeck.command>   get
//         <utility/loopio.ext>
//         <..hyper.request>       repl://get
        
//         + .operation            list
//         <ux/nebudeck>           
//         <..command>             hosts       # List info on hosts found in request history
//         <..command>             methods     # List info on available request methods
//         <..terminal.app>        
//         : .command              hosts       
//         : .command              methods     
//         : .repl                 false
        
//         + .host                 repl
//         ```
//         "#
//     );

//     let engine = Engine::default();
//     let engine = engine.compile(workspace).await;

//     RequestRepl::delegate(
//         Terminal::default(),
//         engine,
//     );
// }

// #[derive(Default)]
// struct RequestRepl {
//     engine: Engine
// }

// impl ControlBus for RequestRepl {
//     fn create(
//         engine: Engine,
//     ) -> Self {

//         for (name, host) in engine.iter_hosts() {
            
//         }

//         RequestRepl { engine }
//     }
// }

// impl TerminalApp for RequestRepl {
//     fn parse_command(&mut self) -> clap::Command {
//         clap::builder::Command::new("test")
//             .multicall(true)
//             .subcommand(clap::builder::Command::new("ping"))
//             .subcommand(clap::builder::Command::new("exit"))
//     }

//     fn enable_repl(&self) -> bool {
//         true
//     }

//     fn on_subcommand(&mut self, name: &str, _: &clap::ArgMatches) {
//         match name {
//             "ping" => {
//                 println!("pong");
//             }
//             "exit" => {
//                 std::process::exit(0);
//             }
//             _ => {}
//         }
//     }

//     fn format_prompt(&mut self) {
//         print!("> ");
//     }

//     fn process_command(self, _: clap::Command) {}
// }

fn main() {}