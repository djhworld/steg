use crate::*;
use flate2::read::GzEncoder;
use flate2::Compression;
use log::debug;
use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;
use std::io::{BufRead, Cursor, Read, Seek, Write};

struct EncodeOutput {
    header: Vec<u8>,
    data: Vec<u8>,
}

impl EncodeOutput {
    fn len(&self) -> usize {
        self.header.len() + self.data.len()
    }
}

pub struct Encoder {
    compress_input: CompressInput,
    byte_split_level: ByteSplitGranularity,
}

impl Encoder {
    pub fn new(compress_input: CompressInput, byte_split_level: ByteSplitGranularity) -> Self {
        Self {
            compress_input,
            byte_split_level,
        }
    }

    pub fn encode<R1: BufRead + Read + Seek, R2: Read, W: Write>(
        &self,
        cover_image: R1,
        input_data: &mut R2,
        output: &mut W,
    ) -> Result<(), std::io::Error> {
        match image::load(cover_image, image::ImageFormat::Png) {
            Ok(img) => {
                let encode_output = if let CompressInput::Gzip = self.compress_input {
                    let compressed = self.compress(input_data)?;
                    self.encode_data(&mut Cursor::new(compressed))
                } else {
                    self.encode_data(input_data)
                }?;

                let rgba8 = img.to_rgba8();

                let mut cover_image_bytes: Vec<u8> =
                    rgba8.bytes().collect::<Result<Vec<u8>, std::io::Error>>()?;

                self.check_utilisation(&cover_image_bytes, &encode_output)?;

                self.merge_into(&mut cover_image_bytes, encode_output);

                let out_buffer = match image::RgbaImage::from_raw(
                    rgba8.width(),
                    rgba8.height(),
                    cover_image_bytes,
                ) {
                    Some(b) => Ok(b),
                    None => Err(err_to_io_error(
                        "could not create output image buffer from raw parts",
                    )),
                }?;

                match image::DynamicImage::ImageRgba8(out_buffer)
                    .write_to(output, image::ImageFormat::Png)
                {
                    Ok(_) => Ok(()),
                    Err(err) => Err(err_to_io_error(err)),
                }
            }
            Err(err) => Err(err_to_io_error(err)),
        }
    }

    // Make sure that we can fit our encoded bytes into the cover image
    fn check_utilisation(
        &self,
        cover_image: &[u8],
        encode_output: &EncodeOutput,
    ) -> Result<(), std::io::Error> {
        let cover_image_size = cover_image.len();
        let input_data_encoded_size = encode_output.len();
        let cover_image_utilisation =
            ((input_data_encoded_size as f64) / (cover_image_size as f64)) * 100.0;

        let info_string = format!(
            "cover image size: {}, encoded_data_size: {}, cover image utilisation: {:.4}%",
            cover_image_size, input_data_encoded_size, cover_image_utilisation,
        );

        debug!("{}", info_string);

        if input_data_encoded_size <= cover_image_size {
            Ok(())
        } else {
            Err(err_to_io_error(format!(
                "cover image is too small for input, perhaps try a different encoding granularity or compress! ({})",
                info_string,
            )))
        }
    }

    fn compress<R: Read>(&self, reader: &mut R) -> Result<Vec<u8>, std::io::Error> {
        let mut compressed_data: Vec<u8> = Vec::new();
        let mut encoder = GzEncoder::new(reader, Compression::default());
        let uncompressed_bytes = encoder.read_to_end(&mut compressed_data)?;
        debug!(
            "compression ratio: {:.4}%",
            ((compressed_data.len() as f64) / (uncompressed_bytes as f64)) * 100.0
        );

        Ok(compressed_data)
    }

    fn encode_data<R: Read>(&self, reader: &mut R) -> Result<EncodeOutput, std::io::Error> {
        let mut out: Vec<u8> = Vec::new();
        let mut header = DataHeader::new(self.compress_input, self.byte_split_level);
        let mut hasher = DefaultHasher::new();
        let mut bytes_count = 0;

        // 1. Explode data into multiple bytes, depending on byte_split_level
        for b in reader.bytes() {
            let bb = b?;
            hasher.write_u8(bb);
            let split = byte_encodings::split_byte(header.granularity, bb);
            out.write_all(&split)?;
            bytes_count += 1;
        }

        header.bytes_count = bytes_count as u64;
        header.data_hash = hasher.finish();
        header.compress_input = self.compress_input;

        debug!("encode header: {:?}", header);

        // Populate header in output
        let raw_header: [u8; HEADER_LENGTH] = header.into();

        Ok(EncodeOutput {
            header: raw_header.to_vec(),
            data: out,
        })
    }

    fn merge_into(&self, dest: &mut [u8], src: EncodeOutput) {
        byte_encodings::BytesZipper::merge_into(
            &mut dest[0..HEADER_LENGTH],
            &src.header,
            ByteSplitGranularity::FourBits,
        );

        let src_size = src.data.len();

        byte_encodings::BytesZipper::merge_into(
            &mut dest[HEADER_LENGTH..(HEADER_LENGTH + src_size)],
            &src.data,
            self.byte_split_level,
        );
    }
}
