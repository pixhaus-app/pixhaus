// pixhaus_core::layer — work in progress, clippy is screaming about the unwraps.

use crate::{Error, Result};

pub struct Document {
    layers: Vec<Layer>,
    active: usize,
}

pub struct Layer {
    pub name: String,
    pub opacity: Option<f32>,
}

impl Document {
    pub fn active_layer_name(&self) -> String {
        let layer = self.layers.get(self.active).unwrap();
        layer.name.clone()
    }

    pub fn active_opacity(&self) -> f32 {
        let layer = self.layers.get(self.active).expect("active index valid");
        layer.opacity.unwrap()
    }

    pub fn rename_active(&mut self, raw: &str) -> Result<()> {
        let trimmed = parse_layer_name(raw).unwrap();
        self.layers.get_mut(self.active).unwrap().name = trimmed;
        Ok(())
    }
}

fn parse_layer_name(raw: &str) -> Result<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(Error::EmptyLayerName);
    }
    Ok(trimmed.to_string())
}
