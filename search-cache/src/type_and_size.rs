use fswalk::NodeFileType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
#[serde(transparent)]
/// state in the high 2 bits, type in the next 2bits, size in the low 44 bits(16T-1 maximum)
pub struct StateTypeSize([u8; 6]);

impl StateTypeSize {
    pub fn none() -> Self {
        assert_eq!(NodeFileType::File as u8, 0);
        Self::new(State::None, NodeFileType::File, 0)
    }

    pub fn unaccessible() -> Self {
        assert_eq!(NodeFileType::File as u8, 0);
        Self::new(State::Unaccessible, NodeFileType::File, 0)
    }

    pub fn some(r#type: NodeFileType, size: u64) -> Self {
        Self::new(State::Some, r#type, size)
    }

    fn new(state: State, r#type: NodeFileType, size: u64) -> Self {
        let bytes = (size.min((1 << 44) - 1) | ((r#type as u64) << 44) | ((state as u64) << 46))
            .to_le_bytes();
        let mut result = [0u8; 6];
        result.copy_from_slice(&bytes[..6]);
        Self(result)
    }

    pub fn state(&self) -> State {
        State::n(self.0[5] >> 6).unwrap()
    }

    pub fn r#type(&self) -> NodeFileType {
        NodeFileType::n(self.0[5] >> 4 & 0b11).unwrap()
    }

    pub fn size(&self) -> u64 {
        let value = u64::from_le_bytes([
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], 0, 0,
        ]);
        value & ((1u64 << 44) - 1)
    }
}

#[derive(Debug, Clone, Copy, enumn::N, PartialEq, Eq)]
#[repr(u8)]
pub enum State {
    Unaccessible = 0,
    Some = 1,
    None = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_and_size() {
        let max_size = (1u64 << 44) - 1;
        let state = State::Some;
        let file_type = NodeFileType::File;
        let ts = StateTypeSize::new(state, file_type, max_size);
        assert_eq!(ts.state(), State::Some);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), max_size);

        let file_type = NodeFileType::Dir;
        let size = 12345;
        let ts = StateTypeSize::new(state, file_type, size);
        assert_eq!(ts.state(), State::Some);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), size);

        let state = State::None;
        let file_type = NodeFileType::Symlink;
        let size = 0;
        let ts = StateTypeSize::new(state, file_type, size);
        assert_eq!(ts.state(), State::None);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), size);

        let state = State::Unaccessible;
        let file_type = NodeFileType::Unknown;
        let size = 987654321;
        let ts = StateTypeSize::new(state, file_type, size);
        assert_eq!(ts.state(), State::Unaccessible);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), size);
    }

    #[test]
    fn test_size_overflow() {
        let too_large_size = 1u64 << 44;
        let state = State::Some;
        let file_type = NodeFileType::File;
        let ts = StateTypeSize::new(state, file_type, too_large_size);
        assert_eq!(ts.state(), State::Some);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), (1 << 44) - 1); // size saturating

        let another_large_size = ((1u64 << 44) - 1) + 100;
        let ts = StateTypeSize::new(state, file_type, another_large_size);
        assert_eq!(ts.state(), State::Some);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), (1 << 44) - 1);

        let max_size = (1 << 44) - 1;
        let ts = StateTypeSize::new(state, file_type, max_size);
        assert_eq!(ts.state(), State::Some);
        assert_eq!(ts.r#type(), file_type);
        assert_eq!(ts.size(), max_size);
    }
}
