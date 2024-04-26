use crate::{errors::OutOfMemoryError, shaders::ShaderError};

mod cache;
mod graphics;

pub use cache::{create_pipeline_cache, save_pipeline_cache, PipelineCacheError};
pub use graphics::GraphicsPipeline;

#[derive(Debug, thiserror::Error)]
pub enum PipelineCreationError {
  #[error("Out of memory")]
  OutOfMemory(#[source] OutOfMemoryError),
  #[error("Failed to load shader \"{1}\"")]
  ShaderFailed(#[source] ShaderError, &'static str),
  #[error("Failed to compile or link shaders")]
  CompilationFailed,
}

impl From<OutOfMemoryError> for PipelineCreationError {
  fn from(value: OutOfMemoryError) -> Self {
    PipelineCreationError::OutOfMemory(value)
  }
}
