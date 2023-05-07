use reality::v2::prelude::*;

use crate::plugin::Plugin;

/// Component that prints messages to stderr and stdout,
///
#[derive(Runmd, Debug, Clone, Component)]
#[storage(specs::VecStorage)]
#[compile(Call, ExampleAPI)]
pub struct Println {
    /// Input to the println component,
    println: String,
    /// Map of properties that can be used to format lines being printed,
    #[config(ext=plugin.map)]
    fmt: Vec<String>,
    /// Lines to print to stderr,
    #[config(ext=plugin.format)]
    stderr: Vec<String>,
    /// Lines to print to stdout,
    #[config(ext=plugin.format)]
    stdout: Vec<String>,
    /// Plugin extension
    #[ext]
    plugin: Plugin,
}

#[async_trait]
impl reality::v2::Call for Println {
    async fn call(&self) -> Result<Properties> {
        for out in self.stdout.iter() {
            self.plugin
                .apply_formatting("fmt", "stdout", out)
                .map_or_else(
                    || {
                        println!("{out}");
                    },
                    |f| {
                        println!("{f}");
                    },
                );
        }

        for err in self.stderr.iter() {
            self.plugin
                .apply_formatting("fmt", "stderr", err)
                .map_or_else(
                    || {
                        eprintln!("{err}");
                    },
                    |f| {
                        eprintln!("{f}");
                    },
                );
        }

        Err(Error::skip())
    }
}

impl ExampleAPI for Println {
    fn example1(&self, properties: &Properties) -> Result<()>  {
        println!("{:?}", self);
        println!("{}", properties);
        Ok(())
    }

    fn example2(&self, identifier: &Identifier) -> Result<()> {
        println!("{:?}", self);
        println!("{:#}", identifier);
        Ok(())
    }

    fn example3(&self, println: &mut Println) -> Result<()>  {
        println!("entering current 3");
        // todo!()
        println.println = String::from("testing current4");
        Ok(())
    }

    fn example4(&self, println: &Println) -> Result<()>  {
        println!("entering current 4");
        println!("{}", println.println);
        Ok(())
    }

    fn example5(&self, println: &Println) -> Result<ThunkExampleAPI> {
        println!("entering current 5");
        let mut pr = println.clone();
        pr.println = String::from("Testing map with");
        Ok(thunk_exampleapi(pr))
    }

    fn example6(&self) -> Result<Println> {
        println!("entering current 6");
        let mut pr = self.clone();
        pr.println = String::from("Testing map");
        Ok(pr)
    }
}

#[thunk]
pub trait ExampleAPI {
    /// This example shows a trait_fn that is converted into a read_with that loads the Properties component,
    /// 
    fn example1(&self, properties: &Properties) -> Result<()>;

    /// This example shows a trait_fn that is converted into a read_with that loads the Identifier component,
    /// 
    fn example2(&self, identifier: &Identifier) -> Result<()>;

    /// This example shows a trait_fn that is converted into a write_with that mutates a Println component,
    /// 
    fn example3(&self, println: &mut Println) -> Result<()>;

    /// This example shows a trait_fn that is converted into a read_with that loads the Println component,
    /// 
    fn example4(&self, println: &Println) -> Result<()>;

    /// This example shows a trait_fn that is converted into a map_with that updates the underlying thunk component,
    /// 
    fn example5(&self, println: &Println) -> Result<ThunkExampleAPI>;

    /// This example shows a trait_fn that is converted into a map that updates the underlying Println component,
    /// 
    fn example6(&self) -> Result<Println>;
}

impl Println {
    /// Returns a new empty Println component,
    ///
    pub const fn new() -> Self {
        Self {
            println: String::new(),
            stderr: vec![],
            stdout: vec![],
            fmt: vec![],
            plugin: Plugin::new(),
        }
    }
}
