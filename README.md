Steganography library.

## Encode 

Provide an image, data to encode and a place to store the result

```rust
    let mut cover =  File::open("image.png")?;
    let mut data = BufReader::new(Cursor::new("Hello world!"));
    let mut encode_output = Vec::new();
    let encoder = Encoder::new(CompressInput::None, ByteSplitGranularity::OneBit);

    encoder
        .encode(&mut cover, &mut data, &mut encode_output)
        .expect("no error");
```

You can optionally compress the data by providing `CompressInput::Gzip`

`ByteSplitGranularity` controls the level of encoding. `OneBit` hides the data in the least significant bit pretty well but consumes a lot of space, and `FourBits` will most likely be noticeable in the resulting image

## Decode

```rust
    let mut image =  File::open("encoded-image.png")?;
    let mut decode_output = Vec::new();
    let decoder = Decoder::new();

    decode
        .decode(&mut image, &mut decode_output)
        .expect("no error");
```
