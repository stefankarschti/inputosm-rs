//! Read OpenStreetMap PBF, XML, and OSC files.

mod inputosm;
pub mod inputosmlog;
pub mod inputosmpbf;
mod inputosmxml;
mod pbfcolumns;
pub mod timeutil;

pub use inputosm::*;
pub use inputosmpbf::{PbfBlock, PbfReader};

/// Callback views cannot escape into caller storage.
///
/// ```compile_fail
/// use inputosm::{NodeOptions, PbfReader, Result};
/// fn retain(reader: &mut PbfReader) -> Result<()> {
///     let mut retained = None;
///     reader.read_block(1, |block| {
///         block.decode_nodes(NodeOptions::default(), |batch| {
///             retained = Some(batch.ids);
///             Ok(())
///         })
///     })?;
///     println!("{:?}", retained);
///     Ok(())
/// }
/// ```
pub mod lifetime_contract {}
