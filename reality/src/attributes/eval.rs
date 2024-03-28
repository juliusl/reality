use std::fmt::Display;

use async_trait::async_trait;
use runmd::prelude::BlockProvider;

use crate::{FieldRefController, NewFn, Plugin};

#[async_trait(?Send)]
pub trait Eval: Plugin + Clone
where
    <Self as Plugin>::Virtual:
        NewFn<Inner = Self> + FieldRefController<Owner = Self> + BlockProvider + Unpin,
{
    /// Evaluates runmd and applies changes to self,
    ///
    async fn eval(&mut self, source: impl Display) -> anyhow::Result<()> {
        use crate::wire::prelude::*;

        let virt = self.clone().to_virtual();

        let owner = virt.send_raw();

        let mut parser = runmd::prelude::Parser::new(virt, ());

        parser.parse(source.to_string()).await;

        *self = owner.borrow().to_owned();

        Ok(())
    }
}

/// Enumeration of types of sources that can be evaluated,
///
pub enum EvalSource<'a> {
    /// Source is runmd, when displayed will be have runmd block
    ///
    Runmd(&'a str),
}

/// Creates an eval source for runmd,
///
/// **Note**: The block delimitters do not need to be included with the input to this function.
///
#[inline]
pub fn runmd(src: &str) -> EvalSource<'_> {
    EvalSource::Runmd(src)
}

impl<'a> Display for EvalSource<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalSource::Runmd(s) => {
                writeln!(f, "```runmd")?;
                writeln!(f, "{}", s.trim())?;
                writeln!(f, "```")?;
            }
        }

        Ok(())
    }
}

mod tests {
    use crate::prelude::*;

    #[derive(Reality, Default, Debug, Clone)]
    #[plugin_def(noop)]
    struct Test {
        #[reality(derive_fromstr, allow_eval)]
        val: String,
        #[reality(allow_eval)]
        decorated_val: Decorated<String>,
    }

    #[tokio::test]
    async fn test_eval_runmd_updates_field() {
        let mut test = Test::default();

        test.eval(runmd_src(": .val hello-world")).await.unwrap();
        assert_eq!("hello-world", test.val);

        test.eval(runmd_src(": .val hello-world-2")).await.unwrap();
        assert_eq!("hello-world-2", test.val);

        test.eval(runmd_src(r#"
        # -- Testing doc header for value
        : test_tag .decorated_val hello-world
        |# test_prop = value
        "#)).await.unwrap();

        assert_eq!("hello-world", test.decorated_val.value().unwrap().as_str());
        assert_eq!("test_tag", test.decorated_val.tag().unwrap().as_str());
        ()
    }
}
