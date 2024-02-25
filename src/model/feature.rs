pub struct Feature(u32);
impl Feature {
    pub fn new(features: u32) -> Self {
        Self(features)
    }
    pub fn has(&self, feature: u32) -> bool {
        self.0 & feature != 0
    }
    pub fn set(&mut self, feature: u32) {
        self.0 |= feature;
    }
    pub fn unset(&mut self, feature: u32) {
        self.0 &= !feature;
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut bytes = self.0.to_be_bytes().to_vec();
        let mut found_non_zero = false;
        bytes.retain(|b| if found_non_zero {
            true
        } else if *b != 0 {
            found_non_zero = true;
            true
        } else {
            false
        });
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > 4 {
            return None;
        }
        let mut num_bytes = [0u8; 4];
        for (idx, b) in bytes.into_iter().rev().enumerate() {
            num_bytes[3-idx] = *b;
        }
        Some(Self(u32::from_be_bytes(num_bytes)))
    }

    pub fn serialize(&self) -> String {
        let bytes = self.bytes();
        hex::encode(&bytes)
            .trim_start_matches('0')
            .to_string()
    }

    pub fn deserialize(s: &str) -> Option<Self> {
        let encoded = if s.len() % 8 != 0 {
            "0".repeat(8 - s.len() % 8) + s
        } else {
            String::from(s)
        };
        let bytes = hex::decode(&encoded).ok()?;

        Self::from_bytes(&bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::constants::features;

    #[test]
    fn test_features() {
        let source = Feature::new(features::CHAN_CHUNKED_STREAM);
        let encoded = source.serialize();
        let decoded = Feature::deserialize(&encoded).expect("Cannot deserialize");
        println!("Encoded: {}", encoded);
        assert_eq!(source.0, decoded.0);
    }
}