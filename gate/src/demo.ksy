meta:
  id: demo
  endian: be
seq:
  - id: magic
    contents: "DEMO"
  - id: version
    type: u2
  - id: kind
    type: u1
    enum: color
  - id: num_items
    type: u4
  - id: items
    type: item
    repeat: expr
    repeat-expr: num_items
  - id: trailer
    type: u1
    if: version > 1
enums:
  color:
    0: red
    1: green
    2: blue
types:
  item:
    seq:
      - id: tag
        type: u1
      - id: body
        type:
          switch-on: tag
          cases:
            0: text_body
            1: num_body
  text_body:
    seq:
      - id: len
        type: u1
      - id: value
        type: str
        size: len
        encoding: ASCII
  num_body:
    seq:
      - id: value
        type: u4
instances:
  computed:
    value: version + 1
