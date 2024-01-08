use loopio::prelude::*;

/// Renders runmd blocks as ui nodes
///
pub struct Layout;

impl Layout {
    /// Render layout of blocks,
    ///
    /// ```runmd
    /// + .input    Port
    /// + .button
    /// ```
    async fn render(workspace: Workspace) -> anyhow::Result<Workspace> {
        let mut project = loopio::prelude::Project::<Shared>::new(Shared::default());

        project.add_block_plugin(Some(""), Some("()"), |_| {});

        project.add_node_plugin("", |_, _, _| {});

        workspace.compile(project).await
    }
}
