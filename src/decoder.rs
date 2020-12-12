use crate::{
    byte_encodings, err_to_io_error, ByteSplitGranularity, CompressInput, DataHeader, HEADER_LENGTH,
};
use flate2::write::GzDecoder;
use log::debug;
use std::collections::hash_map::DefaultHasher;
use std::convert::TryFrom;
use std::hash::Hasher;
use std::io::{BufRead, Read, Seek, Write};

pub struct Decoder {}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

impl Decoder {
    pub fn new() -> Self {
        Self {}
    }

    pub fn decode<R: BufRead + Read + Seek, W: Write>(
        &self,
        input_image: &mut R,
        output: &mut W,
    ) -> Result<(), std::io::Error> {
        match image::load(input_image, image::ImageFormat::Png) {
            Ok(img) => {
                let image_bytes = img
                    .to_rgba8()
                    .bytes()
                    .collect::<Result<Vec<u8>, std::io::Error>>()?;

                let payload = self.uncover_from(image_bytes)?;
                output.write_all(&payload)
            }
            Err(err) => Err(err_to_io_error(err)),
        }
    }

    fn uncover_from(&self, input: Vec<u8>) -> Result<Vec<u8>, std::io::Error> {
        if input.len() < HEADER_LENGTH {
            return Err(err_to_io_error(
                "validation failure: image header is not present",
            ));
        }

        // 1. extract header
        let header = match self.extract_header(&input[0..HEADER_LENGTH]) {
            Ok(h) => Ok(h),
            Err(err) => Err(err_to_io_error(err)),
        }?;

        debug!("decoded header: {:?}", header);

        let minimum_size = match header.granularity {
            ByteSplitGranularity::FourBits => (header.bytes_count as usize * 2),
            ByteSplitGranularity::TwoBits => (header.bytes_count as usize * 4),
            ByteSplitGranularity::OneBit => (header.bytes_count as usize * 8),
        };

        let remaining = &input[HEADER_LENGTH..];

        if remaining.len() < minimum_size {
            return Err(err_to_io_error(
                "validation failure: image data is too small/does not match bytes count in header",
            ));
        }

        let mut data = Vec::new();
        let mut hasher = DefaultHasher::new();

        if header.compress_input == CompressInput::Gzip {
            let mut gzip_decoder = GzDecoder::new(data);

            self.decode_data(remaining, &header, |byte| {
                hasher.write_u8(byte);
                gzip_decoder.write(&[byte])?;
                Ok(())
            })?;

            data = gzip_decoder.finish()?;
        } else {
            self.decode_data(remaining, &header, |byte| {
                hasher.write_u8(byte);
                data.push(byte);
                Ok(())
            })?;
        }

        // 3. validate
        let hash = hasher.finish();
        if hash != header.data_hash {
            return Err(err_to_io_error(format!(
                "validation failure: data hash {} does not match hash printed in header {}",
                hash, header.data_hash
            )));
        }

        Ok(data)
    }

    fn extract_header(&self, input: &[u8]) -> Result<DataHeader, String> {
        let mut raw_header: [u8; HEADER_LENGTH] = [0; HEADER_LENGTH];
        raw_header[..HEADER_LENGTH].copy_from_slice(&input[..]);
        raw_header.iter_mut().for_each(|x| *x &= 0x0F);
        DataHeader::try_from(raw_header)
    }

    fn decode_data<F: FnMut(u8) -> Result<(), std::io::Error>>(
        &self,
        data: &[u8],
        header: &DataHeader,
        handle_byte_fn: F,
    ) -> Result<(), std::io::Error> {
        let chunk_size: usize = match header.granularity {
            ByteSplitGranularity::FourBits => 2,
            ByteSplitGranularity::TwoBits => 4,
            ByteSplitGranularity::OneBit => 8,
        };

        data.chunks(chunk_size)
            .take(header.bytes_count as usize)
            .map(|chunk| byte_encodings::merge_bytes(header.granularity, chunk))
            .map(handle_byte_fn)
            .collect()
    }
}
