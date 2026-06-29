meta:
  id: png
  endian: be
seq:
  - id: magic
    contents: [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]
  - id: ihdr_len
    type: u4
  - id: ihdr_type
    contents: "IHDR"
  - id: ihdr
    type: ihdr_chunk
    size: ihdr_len
  - id: ihdr_crc
    type: u4
  - id: chunks
    type: chunk
    repeat: eos
types:
  ihdr_chunk:
    seq:
      - id: width
        type: u4
      - id: height
        type: u4
      - id: bit_depth
        type: u1
      - id: color_type
        type: u1
        enum: color_type
      - id: compression_method
        type: u1
      - id: filter_method
        type: u1
      - id: interlace_method
        type: u1
  chunk:
    seq:
      - id: len
        type: u4
      - id: type
        type: str
        size: 4
        encoding: ASCII
      - id: body
        size: len
      - id: crc
        type: u4
enums:
  color_type:
    0: greyscale
    2: truecolor
    3: indexed
    6: truecolor_alpha
