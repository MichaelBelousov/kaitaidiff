meta:
  id: sqlite3
  title: SQLite 3 database header
  endian: be
doc: |
  Parses the 100-byte database header of a SQLite 3 file. This is enough to
  show meaningful semantic diffs (change counters, page counts, schema cookie,
  user_version, ...) without decoding the b-tree pages, which would require the
  higher-level, app-specific traversal mentioned in the kdiff roadmap.
seq:
  - id: magic
    contents: ["SQLite format 3", 0]
  - id: page_size
    type: u2
  - id: write_version
    type: u1
    enum: file_format_version
  - id: read_version
    type: u1
    enum: file_format_version
  - id: reserved_space
    type: u1
  - id: max_payload_fraction
    type: u1
  - id: min_payload_fraction
    type: u1
  - id: leaf_payload_fraction
    type: u1
  - id: file_change_counter
    type: u4
  - id: database_size_in_pages
    type: u4
  - id: first_freelist_trunk_page
    type: u4
  - id: total_freelist_pages
    type: u4
  - id: schema_cookie
    type: u4
  - id: schema_format_number
    type: u4
  - id: default_page_cache_size
    type: u4
  - id: largest_root_btree_page
    type: u4
  - id: text_encoding
    type: u4
    enum: text_encoding
  - id: user_version
    type: u4
  - id: incremental_vacuum_mode
    type: u4
  - id: application_id
    type: u4
  - id: reserved
    size: 20
  - id: version_valid_for
    type: u4
  - id: sqlite_version_number
    type: u4
enums:
  file_format_version:
    1: legacy
    2: wal
  text_encoding:
    1: utf8
    2: utf16le
    3: utf16be
