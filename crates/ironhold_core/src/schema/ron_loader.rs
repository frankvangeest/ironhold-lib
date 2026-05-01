use bevy::asset::{Asset, AssetApp, AssetLoader, LoadContext, io::Reader};
use bevy::app::{App, Plugin};
use bevy::reflect::TypePath;
use ron::extensions::Extensions;
use serde::Deserialize;
use std::marker::PhantomData;

/// Bevy plugin that registers a RON asset loader with `implicit_some` enabled globally.
/// Designers can write `radius: 0.4` instead of `radius: Some(0.4)` in any `.ron` file.
pub struct ImplicitRonPlugin<A> {
    extensions: Vec<&'static str>,
    _marker: PhantomData<A>,
}

impl<A> ImplicitRonPlugin<A>
where
    for<'de> A: Deserialize<'de> + Asset,
{
    pub fn new(extensions: &[&'static str]) -> Self {
        Self {
            extensions: extensions.to_owned(),
            _marker: PhantomData,
        }
    }
}

impl<A> Plugin for ImplicitRonPlugin<A>
where
    for<'de> A: Deserialize<'de> + Asset,
{
    fn build(&self, app: &mut App) {
        app.init_asset::<A>()
            .register_asset_loader(ImplicitRonLoader::<A> {
                extensions: self.extensions.clone(),
                _marker: PhantomData,
            });
    }
}

/// Possible errors from [`ImplicitRonLoader`].
#[derive(Debug)]
pub enum RonLoadError {
    Io(std::io::Error),
    Ron(ron::error::SpannedError),
}

impl std::fmt::Display for RonLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error reading RON asset: {e}"),
            Self::Ron(e) => write!(f, "RON parse error: {e}"),
        }
    }
}

impl std::error::Error for RonLoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Ron(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for RonLoadError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}

impl From<ron::error::SpannedError> for RonLoadError {
    fn from(e: ron::error::SpannedError) -> Self { Self::Ron(e) }
}

/// RON asset loader with `implicit_some` enabled — every `Option<T>` field can be
/// written as a bare value (`radius: 0.4`) rather than `radius: Some(0.4)`.
#[derive(TypePath)]
pub struct ImplicitRonLoader<A> {
    extensions: Vec<&'static str>,
    _marker: PhantomData<A>,
}

impl<A> AssetLoader for ImplicitRonLoader<A>
where
    for<'de> A: Deserialize<'de> + Asset,
{
    type Asset = A;
    type Settings = ();
    type Error = RonLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        let opts = ron::Options::default()
            .with_default_extension(Extensions::IMPLICIT_SOME);
        Ok(opts.from_bytes::<A>(&bytes)?)
    }

    fn extensions(&self) -> &[&str] {
        &self.extensions
    }
}
