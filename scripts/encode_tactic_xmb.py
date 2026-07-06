#!/usr/bin/env python3
"""
Encode an XML file to AoM:R's XMB binary format.

Usage:
    python3 encode_tactic_xmb.py <input.xml> <output.XMB>

The encoder follows the same algorithm as CryBarEditor's
CryBar/Bar/BarFormatConverter.cs XMLtoXMB.

Output is raw X1 (no alz4 wrapping). Modded .XMB files in mod folders are
also stored as raw X1 — the alz4 wrapping only happens inside Data.bar for
storage compression.

File format (little-endian, all offsets/sizes int32 unless noted):
  - "X1" magic (2 bytes)
  - data_length (int32) — bytes after this field
  - "XR" magic (2 bytes)
  - id1 = 4 (int32)
  - version = 8 (int32)
  - n_elements (int32)
  - For each element: length in chars (int32), name UTF-16LE
  - n_attributes (int32)
  - For each attribute: length in chars (int32), name UTF-16LE
  - Recursive node records:
      - "XN" magic (2 bytes)
      - node_length (int32, backfilled) — bytes until end of this node
      - text_length in chars (int32), text UTF-16LE if length > 0
      - element_index (int32)
      - line_number = 0 (int32)
      - n_attributes (int32)
      - For each attribute: attr_index (int32), text_length in chars (int32), text UTF-16LE
      - n_child_elements (int32, only Element children, no text/comment nodes counted)
      - Recursive child nodes
"""
import io
import struct
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def _encode_xml_root(root: ET.Element) -> bytes:
    """Encode an ElementTree root into the X1 binary stream (pre-compression)."""
    # Build element and attribute name tables in DFS order.
    elements: list[str] = []
    element_index: dict[str, int] = {}
    attributes: list[str] = []
    attribute_index: dict[str, int] = {}

    def find_names(node: ET.Element) -> None:
        if node.tag not in element_index:
            element_index[node.tag] = len(elements)
            elements.append(node.tag)
        for attr_name in node.attrib:
            if attr_name not in attribute_index:
                attribute_index[attr_name] = len(attributes)
                attributes.append(attr_name)
        for child in node:
            find_names(child)

    find_names(root)

    buf = io.BytesIO()

    # X1 magic (2 bytes)
    buf.write(b"X1")

    # Data length placeholder (int32) — backfilled at end
    data_length_pos = buf.tell()
    buf.write(struct.pack("<i", 0))

    # XR magic (2 bytes)
    buf.write(b"XR")

    # id1 = 4
    buf.write(struct.pack("<i", 4))

    # version = 8
    buf.write(struct.pack("<i", 8))

    # Element table
    buf.write(struct.pack("<i", len(elements)))
    for name in elements:
        encoded = name.encode("utf-16le")
        buf.write(struct.pack("<i", len(encoded) // 2))
        buf.write(encoded)

    # Attribute table
    buf.write(struct.pack("<i", len(attributes)))
    for name in attributes:
        encoded = name.encode("utf-16le")
        buf.write(struct.pack("<i", len(encoded) // 2))
        buf.write(encoded)

    def write_node(node: ET.Element) -> None:
        # XN magic (2 bytes)
        buf.write(b"XN")

        # Node length placeholder (int32) — backfilled at end
        node_length_pos = buf.tell()
        buf.write(struct.pack("<i", 0))
        node_start = buf.tell()

        # Inner text (only direct text children, no element children as text).
        # XML.etree puts text into .text when it's the only child or
        # before any element. For our simple tactic files, .text is enough.
        # Strip surrounding whitespace to match vanilla's compact encoding.
        text = (node.text or "").strip()
        if text:
            encoded = text.encode("utf-16le")
            buf.write(struct.pack("<i", len(encoded) // 2))
            buf.write(encoded)
        else:
            buf.write(struct.pack("<i", 0))

        # Tail text (between this node's closing and the next sibling's opening).
        # Vanilla tactic files don't use tail text, so we ignore it.
        # If node.tail exists and is non-empty, we'd need to encode it, but
        # for our use case it's always whitespace.

        # Element index
        buf.write(struct.pack("<i", element_index[node.tag]))

        # Line number (always 0 for our purposes)
        buf.write(struct.pack("<i", 0))

        # Attributes
        buf.write(struct.pack("<i", len(node.attrib)))
        for attr_name, attr_value in node.attrib.items():
            buf.write(struct.pack("<i", attribute_index[attr_name]))
            encoded = attr_value.encode("utf-16le")
            buf.write(struct.pack("<i", len(encoded) // 2))
            buf.write(encoded)

        # Element children only (no text/comment nodes counted)
        children = list(node)
        buf.write(struct.pack("<i", len(children)))
        for child in children:
            write_node(child)

        # Backfill node_length
        node_end = buf.tell()
        node_length = node_end - node_start
        buf.seek(node_length_pos)
        buf.write(struct.pack("<i", node_length))
        buf.seek(node_end)

    write_node(root)

    # Backfill data_length = total bytes after the length field
    raw = buf.getvalue()
    data_length = len(raw) - 6
    final = bytearray(raw)
    struct.pack_into("<i", final, data_length_pos, data_length)
    return bytes(final)


def encode(xml_path: Path, xmb_path: Path) -> None:
    tree = ET.parse(xml_path)
    raw = _encode_xml_root(tree.getroot())
    xmb_path.write_bytes(raw)
    print(f"encoded {xml_path.name} -> {xmb_path.name} ({len(raw)} bytes)")


def main() -> int:
    if len(sys.argv) != 3:
        print("usage: encode_tactic_xmb.py <input.xml> <output.XMB>", file=sys.stderr)
        return 2
    encode(Path(sys.argv[1]), Path(sys.argv[2]))
    return 0


if __name__ == "__main__":
    sys.exit(main())