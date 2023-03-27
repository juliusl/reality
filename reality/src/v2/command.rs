use std::path::Path;
use std::path::PathBuf;
use specs::Entity;
use specs::WorldExt;
use tracing::info;

use crate::Error;
use super::Compiler;
use super::data::toml::TomlProperties;
use super::toml::DocumentBuilder;


/// Imports a toml document as a build,
/// 
pub async fn import_toml(compiler: &mut Compiler, path: impl AsRef<Path>) -> Result<Entity, Error> {
    info!("Importing toml doc from: {:?}", path.as_ref());
    let properties = TomlProperties::try_load(path).await?;

    let build = compiler.lazy_build(&properties)?;
    compiler.as_mut().maintain();
    
    info!("Imported build to entity: {:?}", build);
    compiler.push_build(build);
    Ok(build)
}

/// Exports world state as a toml document,
/// 
pub async fn export_toml(compiler: &mut Compiler, path: impl Into<PathBuf>) -> Result<(), Error> {
    let path = path.into();

    compiler.last_build().map(|l| {
        info!("Exporting toml for build {:?} to: {:?}", l, &path);
    });

    compiler
        .update_last_build(&mut DocumentBuilder::new())
        .map_into::<TomlProperties>(|p| Ok(p.into()))
        .enable_async()
        .read(|toml| {
            let toml = toml.clone();
            async move {
                toml.try_save(&path).await?;
                info!("Exported toml to: {:?}", &path);
                Ok(())
            }
        }).await.result()?;

    Ok(())
}