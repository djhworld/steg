pub mod decoder;
pub mod encoder;

use std::convert::TryFrom;
use std::convert::TryInto;

const VERSION: u8 = 0x1;
const MAGIC: u16 = 0xBEAD;
const HEADER_LENGTH: usize = 40;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ByteSplitGranularity {
    FourBits,
    TwoBits,
    OneBit,
}

impl TryFrom<u8> for ByteSplitGranularity {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            4 => Ok(ByteSplitGranularity::FourBits),
            2 => Ok(ByteSplitGranularity::TwoBits),
            1 => Ok(ByteSplitGranularity::OneBit),
            _ => Err("Unsupported value for ByteSplitGranularity".to_string()),
        }
    }
}

impl Into<u8> for ByteSplitGranularity {
    fn into(self) -> u8 {
        match self {
            ByteSplitGranularity::FourBits => 4,
            ByteSplitGranularity::TwoBits => 2,
            ByteSplitGranularity::OneBit => 1,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum CompressInput {
    None,
    Gzip,
}

impl TryFrom<u8> for CompressInput {
    type Error = String;

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(CompressInput::None),
            1 => Ok(CompressInput::Gzip),
            _ => Err("Unsupported value for CompressInput".to_string()),
        }
    }
}

impl Into<u8> for CompressInput {
    fn into(self) -> u8 {
        match self {
            CompressInput::None => 0,
            CompressInput::Gzip => 1,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DataHeader {
    magic: u16,
    version: u8,
    bytes_count: u64,
    data_hash: u64,
    compress_input: CompressInput,
    granularity: ByteSplitGranularity,
}

impl DataHeader {
    pub fn new(compress_input: CompressInput, granularity: ByteSplitGranularity) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            bytes_count: 0,
            data_hash: 0,
            compress_input,
            granularity,
        }
    }
}

impl Default for DataHeader {
    fn default() -> Self {
        Self::new(CompressInput::None, ByteSplitGranularity::FourBits)
    }
}

impl TryFrom<[u8; HEADER_LENGTH]> for DataHeader {
    type Error = String;
    fn try_from(data: [u8; HEADER_LENGTH]) -> Result<Self, Self::Error> {
        let mut magic_expanded: [u8; 16] = [0; 16];
        magic_expanded[12..16].clone_from_slice(&data[0..4]);

        let magic: u64 = NibbleNumber::new(magic_expanded).into();
        let magic: u16 = magic as u16;

        if magic != MAGIC {
            return Err(format!("invalid magic: {:#x}", magic));
        }

        let mut version_expanded: [u8; 16] = [0; 16];
        version_expanded[14..16].clone_from_slice(&data[4..6]);

        let version: u64 = NibbleNumber::new(version_expanded).into();
        let version: u8 = version as u8;

        if version != VERSION {
            return Err(format!("unsupported version: {:#x}", version));
        }

        let bytes_count: u64 = NibbleNumber::new(data[6..22].to_vec().try_into().unwrap()).into();
        let data_hash: u64 = NibbleNumber::new(data[22..38].to_vec().try_into().unwrap()).into();

        let mut compressed_expanded: [u8; 16] = [0; 16];
        compressed_expanded[15..16].clone_from_slice(&data[38..39]);

        let compressed_expanded: u64 = NibbleNumber::new(compressed_expanded).into();
        let compress_input = CompressInput::try_from(compressed_expanded as u8)?;

        let mut granularity_expanded: [u8; 16] = [0; 16];
        granularity_expanded[15..16].clone_from_slice(&data[39..40]);

        let granularity_expanded: u64 = NibbleNumber::new(granularity_expanded).into();
        let granularity = ByteSplitGranularity::try_from(granularity_expanded as u8)?;

        Ok(DataHeader {
            magic,
            version,
            bytes_count,
            data_hash,
            compress_input,
            granularity,
        })
    }
}

impl Into<[u8; HEADER_LENGTH]> for DataHeader {
    fn into(self) -> [u8; HEADER_LENGTH] {
        let magic = NibbleNumber::from(self.magic as u64);
        let version = NibbleNumber::from(self.version as u64);
        let bytes_count = NibbleNumber::from(self.bytes_count);
        let hash = NibbleNumber::from(self.data_hash);

        let compress_input: u8 = self.compress_input.into();
        let compress_input = NibbleNumber::from(compress_input as u64);
        let granularity: u8 = self.granularity.into();
        let granularity = NibbleNumber::from(granularity as u64);

        let mut raw: [u8; HEADER_LENGTH] = [0; HEADER_LENGTH];

        raw[..4].clone_from_slice(&magic.data[12..16]);
        raw[4..6].clone_from_slice(&version.data[14..16]);
        raw[6..22].clone_from_slice(&bytes_count.data);
        raw[22..38].clone_from_slice(&hash.data);
        raw[38..39].clone_from_slice(&compress_input.data[15..16]);
        raw[39..40].clone_from_slice(&granularity.data[15..16]);
        raw
    }
}

fn err_to_io_error<E>(error: E) -> std::io::Error
where
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    std::io::Error::new(std::io::ErrorKind::Other, error.into())
}

#[derive(Debug)]
struct NibbleNumber {
    data: [u8; 16],
}
impl NibbleNumber {
    fn new(data: [u8; 16]) -> Self {
        Self { data }
    }
}

impl From<usize> for NibbleNumber {
    fn from(value: usize) -> Self {
        NibbleNumber::from(value as u64)
    }
}

impl From<u64> for NibbleNumber {
    fn from(value: u64) -> Self {
        let result: [u8; 16] = [
            ((value & 0xF000000000000000) >> 60) as u8,
            ((value & 0x0F00000000000000) >> 56) as u8,
            ((value & 0x00F0000000000000) >> 52) as u8,
            ((value & 0x000F000000000000) >> 48) as u8,
            ((value & 0x0000F00000000000) >> 44) as u8,
            ((value & 0x00000F0000000000) >> 40) as u8,
            ((value & 0x000000F000000000) >> 36) as u8,
            ((value & 0x0000000F00000000) >> 32) as u8,
            ((value & 0x00000000F0000000) >> 28) as u8,
            ((value & 0x000000000F000000) >> 24) as u8,
            ((value & 0x0000000000F00000) >> 20) as u8,
            ((value & 0x00000000000F0000) >> 16) as u8,
            ((value & 0x000000000000F000) >> 12) as u8,
            ((value & 0x0000000000000F00) >> 8) as u8,
            ((value & 0x00000000000000F0) >> 4) as u8,
            (value & 0x000000000000000F) as u8,
        ];

        NibbleNumber { data: result }
    }
}

impl Into<u64> for NibbleNumber {
    fn into(self) -> u64 {
        let value_count: u64 = (self.data[0] as u64) << 60
            | (self.data[1] as u64) << 56
            | (self.data[2] as u64) << 52
            | (self.data[3] as u64) << 48
            | (self.data[4] as u64) << 44
            | (self.data[5] as u64) << 40
            | (self.data[6] as u64) << 36
            | (self.data[7] as u64) << 32
            | (self.data[8] as u64) << 28
            | (self.data[9] as u64) << 24
            | (self.data[10] as u64) << 20
            | (self.data[11] as u64) << 16
            | (self.data[12] as u64) << 12
            | (self.data[13] as u64) << 8
            | (self.data[14] as u64) << 4
            | (self.data[15] as u64);

        value_count
    }
}

mod byte_encodings {
    use super::ByteSplitGranularity;

    pub struct BytesZipper {}

    impl BytesZipper {
        pub fn merge_into(dest: &mut [u8], src: &[u8], granularity: ByteSplitGranularity) {
            dest.iter_mut().zip(src).for_each(|(left, right)| {
                *left = zip_bytes(granularity, left.clone(), *right);
            });
        }
    }

    pub fn split_byte(granularity: ByteSplitGranularity, byte: u8) -> Vec<u8> {
        match granularity {
            ByteSplitGranularity::FourBits => vec![byte >> 4, byte & 0x0F],
            ByteSplitGranularity::TwoBits => vec![
                (byte >> 6) & 0x03,
                (byte >> 4) & 0x03,
                (byte >> 2) & 0x03,
                byte & 0x03,
            ],
            ByteSplitGranularity::OneBit => vec![
                (byte >> 7) & 0x01,
                (byte >> 6) & 0x01,
                (byte >> 5) & 0x01,
                (byte >> 4) & 0x01,
                (byte >> 3) & 0x01,
                (byte >> 2) & 0x01,
                (byte >> 1) & 0x01,
                byte & 0x01,
            ],
        }
    }

    pub fn zip_bytes(granularity: ByteSplitGranularity, left: u8, right: u8) -> u8 {
        match granularity {
            ByteSplitGranularity::FourBits => (left & 0xF0) | (right & 0x0F),
            ByteSplitGranularity::TwoBits => (left & 0xFC) | (right & 0x03),
            ByteSplitGranularity::OneBit => (left & 0xFE) | (right & 0x01),
        }
    }

    // FourBits = bottom 4 bits
    // TwoBits = bottom 2 bits
    pub fn merge_bytes(granularity: ByteSplitGranularity, bytes: &[u8]) -> u8 {
        match granularity {
            ByteSplitGranularity::FourBits => ((bytes[0] << 4) & 0xF0) | (bytes[1] & 0x0F),
            ByteSplitGranularity::TwoBits => {
                (bytes[0] << 6) & 0xC0
                    | (bytes[1] << 4) & 0x30
                    | (bytes[2] << 2) & 0x0C
                    | (bytes[3] & 0x03)
            }
            ByteSplitGranularity::OneBit => {
                (bytes[0] << 7) & 0x80
                    | (bytes[1] << 6) & 0x40
                    | (bytes[2] << 5) & 0x20
                    | (bytes[3] << 4) & 0x10
                    | (bytes[4] << 3) & 0x08
                    | (bytes[5] << 2) & 0x04
                    | (bytes[6] << 1) & 0x02
                    | (bytes[7] & 0x01)
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::super::ByteSplitGranularity;
        use super::*;

        #[test]
        fn test_bytes_zipper_one_bit() {
            let mut dest: Vec<u8> = vec![0xFE, 0xFE];
            let src: Vec<u8> = vec![0x01, 0x01];

            BytesZipper::merge_into(&mut dest, &src, ByteSplitGranularity::OneBit);
            assert_eq!(vec![0xFF, 0xFF], dest);
        }

        #[test]
        fn test_bytes_zipper_two_bits() {
            let mut dest: Vec<u8> = vec![0xFC, 0xFC];
            let src: Vec<u8> = vec![0x03, 0x03];

            BytesZipper::merge_into(&mut dest, &src, ByteSplitGranularity::TwoBits);
            assert_eq!(vec![0xFF, 0xFF], dest);
        }

        #[test]
        fn test_bytes_zipper_four_bits() {
            let mut dest: Vec<u8> = vec![0xFF, 0xFF];
            let src: Vec<u8> = vec![0x07, 0x07];

            BytesZipper::merge_into(&mut dest, &src, ByteSplitGranularity::FourBits);
            assert_eq!(vec![0xF7, 0xF7], dest);
        }

        #[test]
        fn test_split_merge_four_bits() {
            test_split_merge(0xFF, vec![0x0F, 0x0F], ByteSplitGranularity::FourBits);
            test_split_merge(0x0F, vec![0x0, 0x0F], ByteSplitGranularity::FourBits);
            test_split_merge(0x4, vec![0x0, 0x04], ByteSplitGranularity::FourBits);
            test_split_merge(0x3, vec![0x0, 0x03], ByteSplitGranularity::FourBits);
            test_split_merge(0x2, vec![0x0, 0x02], ByteSplitGranularity::FourBits);
            test_split_merge(0x1, vec![0x0, 0x01], ByteSplitGranularity::FourBits);
            test_split_merge(0x0, vec![0x0, 0x0], ByteSplitGranularity::FourBits);
            test_split_merge(0xA, vec![0x0, 0x0A], ByteSplitGranularity::FourBits);
            test_split_merge(0x11, vec![0x01, 0x01], ByteSplitGranularity::FourBits);
            test_split_merge(0xAF, vec![0x0A, 0x0F], ByteSplitGranularity::FourBits);
        }

        #[test]
        fn test_split_merge_two_bits() {
            test_split_merge(
                0xFF,
                vec![0x03, 0x03, 0x03, 0x03],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0x03,
                vec![0x00, 0x00, 0x00, 0x03],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0x02,
                vec![0x00, 0x00, 0x00, 0x02],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0x01,
                vec![0x00, 0x00, 0x00, 0x01],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0x0F,
                vec![0x00, 0x00, 0x03, 0x03],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0x11,
                vec![0x00, 0x01, 0x00, 0x01],
                ByteSplitGranularity::TwoBits,
            );
            test_split_merge(
                0xEC,
                vec![0x03, 0x02, 0x03, 0x00],
                ByteSplitGranularity::TwoBits,
            );
        }

        #[test]
        fn test_split_merge_one_bit() {
            test_split_merge(
                0xFF,
                vec![0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x03,
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x01],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x02,
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x01,
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x00,
                vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x0F,
                vec![0x00, 0x00, 0x00, 0x00, 0x01, 0x01, 0x01, 0x01],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0x11,
                vec![0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01],
                ByteSplitGranularity::OneBit,
            );
            test_split_merge(
                0xEC,
                vec![0x01, 0x01, 0x01, 0x00, 0x01, 0x01, 0x00, 0x00],
                ByteSplitGranularity::OneBit,
            );
        }

        #[test]
        fn test_zip_four_bits() {
            test_zip(0xF7, 0x3C, 0xFC, ByteSplitGranularity::FourBits);
            test_zip(0x0, 0xAF, 0x0F, ByteSplitGranularity::FourBits);
            test_zip(0x0, 0x0, 0x0, ByteSplitGranularity::FourBits);
            test_zip(0xA9, 0x19, 0xA9, ByteSplitGranularity::FourBits);
            test_zip(0xFF, 0xFF, 0xFF, ByteSplitGranularity::FourBits);
        }

        #[test]
        fn test_zip_two_bits() {
            test_zip(0xF7, 0x1, 0xF5, ByteSplitGranularity::TwoBits);
            test_zip(0xF3, 0x3, 0xF3, ByteSplitGranularity::TwoBits);
            test_zip(0x0, 0x3, 0x3, ByteSplitGranularity::TwoBits);
            test_zip(0x3, 0x0, 0x0, ByteSplitGranularity::TwoBits);
        }

        #[test]
        fn test_zip_one_bit() {
            test_zip(0xF7, 0x1, 0xF7, ByteSplitGranularity::OneBit);
            test_zip(0xF8, 0x1, 0xF9, ByteSplitGranularity::OneBit);
            test_zip(0x0, 0x1, 0x1, ByteSplitGranularity::OneBit);
            test_zip(0x1, 0x0, 0x0, ByteSplitGranularity::OneBit);
        }

        fn test_zip(left: u8, right: u8, expected: u8, granularity: ByteSplitGranularity) {
            let result = zip_bytes(granularity, left, right);
            assert_eq!(expected, result);
        }

        fn test_split_merge(input: u8, expected: Vec<u8>, granularity: ByteSplitGranularity) {
            let result = split_byte(granularity, input);
            assert_eq!(expected.len(), result.len());
            assert_eq!(expected, result);

            let merged = merge_bytes(granularity, &result);
            assert_eq!(input, merged);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::decoder::*;
    use super::encoder::*;
    use super::{ByteSplitGranularity, CompressInput};
    use std::io::{BufReader, Cursor};

    #[test]
    fn test_encode_decode() {
        // PNG image containing the encoded string "HELLO"
        let image: [u8; 548] = [
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x58, 0x1b, 0xb9, 0x08, 0x00, 0x00, 0x01, 0x83, 0x69, 0x43, 0x43, 0x50, 0x49,
            0x43, 0x43, 0x20, 0x70, 0x72, 0x6f, 0x66, 0x69, 0x6c, 0x65, 0x00, 0x00, 0x28, 0x91,
            0x7d, 0x91, 0x3d, 0x48, 0xc3, 0x40, 0x1c, 0xc5, 0x5f, 0x53, 0xa5, 0x45, 0x2a, 0x0e,
            0x76, 0x10, 0x71, 0xc8, 0x50, 0x9d, 0x2c, 0x88, 0x4a, 0x71, 0xd4, 0x2a, 0x14, 0xa1,
            0x42, 0xa8, 0x15, 0x5a, 0x75, 0x30, 0xb9, 0xf4, 0x0b, 0x9a, 0x34, 0x24, 0x29, 0x2e,
            0x8e, 0x82, 0x6b, 0xc1, 0xc1, 0x8f, 0xc5, 0xaa, 0x83, 0x8b, 0xb3, 0xae, 0x0e, 0xae,
            0x82, 0x20, 0xf8, 0x01, 0xe2, 0xe4, 0xe8, 0xa4, 0xe8, 0x22, 0x25, 0xfe, 0x2f, 0x29,
            0xb4, 0x88, 0xf1, 0xe0, 0xb8, 0x1f, 0xef, 0xee, 0x3d, 0xee, 0xde, 0x01, 0x42, 0xb3,
            0xca, 0x34, 0xab, 0x67, 0x02, 0xd0, 0x74, 0xdb, 0xcc, 0xa4, 0x92, 0x62, 0x2e, 0xbf,
            0x2a, 0x86, 0x5e, 0x11, 0x41, 0x18, 0x51, 0x24, 0x10, 0x90, 0x99, 0x65, 0xcc, 0x49,
            0x52, 0x1a, 0xbe, 0xe3, 0xeb, 0x1e, 0x01, 0xbe, 0xde, 0xc5, 0x79, 0x96, 0xff, 0xb9,
            0x3f, 0x47, 0xbf, 0x5a, 0xb0, 0x18, 0x10, 0x10, 0x89, 0x67, 0x99, 0x61, 0xda, 0xc4,
            0x1b, 0xc4, 0x89, 0x4d, 0xdb, 0xe0, 0xbc, 0x4f, 0x1c, 0x65, 0x65, 0x59, 0x25, 0x3e,
            0x27, 0x1e, 0x37, 0xe9, 0x82, 0xc4, 0x8f, 0x5c, 0x57, 0x3c, 0x7e, 0xe3, 0x5c, 0x72,
            0x59, 0xe0, 0x99, 0x51, 0x33, 0x9b, 0x99, 0x27, 0x8e, 0x12, 0x8b, 0xa5, 0x2e, 0x56,
            0xba, 0x98, 0x95, 0x4d, 0x8d, 0x78, 0x9a, 0x38, 0xa6, 0x6a, 0x3a, 0xe5, 0x0b, 0x39,
            0x8f, 0x55, 0xce, 0x5b, 0x9c, 0xb5, 0x6a, 0x9d, 0xb5, 0xef, 0xc9, 0x5f, 0x18, 0x29,
            0xe8, 0x2b, 0xcb, 0x5c, 0xa7, 0x39, 0x82, 0x14, 0x16, 0xb1, 0x04, 0x09, 0x22, 0x14,
            0xd4, 0x51, 0x41, 0x15, 0x36, 0xe2, 0xb4, 0xea, 0xa4, 0x58, 0xc8, 0xd0, 0x7e, 0xd2,
            0xc7, 0x3f, 0xec, 0xfa, 0x25, 0x72, 0x29, 0xe4, 0xaa, 0x80, 0x91, 0x63, 0x01, 0x35,
            0x68, 0x90, 0x5d, 0x3f, 0xf8, 0x1f, 0xfc, 0xee, 0xd6, 0x2a, 0x4e, 0x4d, 0x7a, 0x49,
            0x91, 0x24, 0xd0, 0xfb, 0xe2, 0x38, 0x1f, 0xa3, 0x40, 0x68, 0x17, 0x68, 0x35, 0x1c,
            0xe7, 0xfb, 0xd8, 0x71, 0x5a, 0x27, 0x40, 0xf0, 0x19, 0xb8, 0xd2, 0x3b, 0xfe, 0x5a,
            0x13, 0x98, 0xf9, 0x24, 0xbd, 0xd1, 0xd1, 0x62, 0x47, 0xc0, 0xc0, 0x36, 0x70, 0x71,
            0xdd, 0xd1, 0x94, 0x3d, 0xe0, 0x72, 0x07, 0x18, 0x7a, 0x32, 0x64, 0x53, 0x76, 0xa5,
            0x20, 0x4d, 0xa1, 0x58, 0x04, 0xde, 0xcf, 0xe8, 0x9b, 0xf2, 0xc0, 0xe0, 0x2d, 0xd0,
            0xb7, 0xe6, 0xf5, 0xd6, 0xde, 0xc7, 0xe9, 0x03, 0x90, 0xa5, 0xae, 0xd2, 0x37, 0xc0,
            0xc1, 0x21, 0x30, 0x56, 0xa2, 0xec, 0x75, 0x9f, 0x77, 0x87, 0xbb, 0x7b, 0xfb, 0xf7,
            0x4c, 0xbb, 0xbf, 0x1f, 0x57, 0xce, 0x72, 0x9c, 0xf7, 0xbf, 0xe8, 0x9e, 0x00, 0x00,
            0x00, 0x09, 0x70, 0x48, 0x59, 0x73, 0x00, 0x00, 0x2e, 0x23, 0x00, 0x00, 0x2e, 0x23,
            0x01, 0x78, 0xa5, 0x3f, 0x76, 0x00, 0x00, 0x00, 0x07, 0x74, 0x49, 0x4d, 0x45, 0x07,
            0xe4, 0x0c, 0x08, 0x15, 0x07, 0x0c, 0x1d, 0x5f, 0x8d, 0xad, 0x00, 0x00, 0x00, 0x19,
            0x74, 0x45, 0x58, 0x74, 0x43, 0x6f, 0x6d, 0x6d, 0x65, 0x6e, 0x74, 0x00, 0x43, 0x72,
            0x65, 0x61, 0x74, 0x65, 0x64, 0x20, 0x77, 0x69, 0x74, 0x68, 0x20, 0x47, 0x49, 0x4d,
            0x50, 0x57, 0x81, 0x0e, 0x17, 0x00, 0x00, 0x00, 0x0f, 0x49, 0x44, 0x41, 0x54, 0x08,
            0xd7, 0x63, 0xfc, 0xff, 0xff, 0x3f, 0x03, 0x29, 0x00, 0x00, 0x8c, 0xd5, 0x02, 0xff,
            0x2f, 0xcb, 0x21, 0xd3, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42,
            0x60, 0x82,
        ];

        let mut cover = BufReader::new(Cursor::new(image.to_vec()));
        let mut data = BufReader::new(Cursor::new("Hey!"));
        let mut encode_output: Vec<u8> = Vec::new();

        let encoder = Encoder::new(CompressInput::None, ByteSplitGranularity::TwoBits);

        encoder
            .encode(&mut cover, &mut data, &mut encode_output)
            .expect("no error");

        let mut decode_input = BufReader::new(Cursor::new(encode_output));
        let mut decode_output: Vec<u8> = Vec::new();

        let decoder = Decoder::new();

        decoder
            .decode(&mut decode_input, &mut decode_output)
            .expect("no error");

        assert_eq!(
            String::from("Hey!"),
            String::from_utf8(decode_output).unwrap(),
        );
    }
}
