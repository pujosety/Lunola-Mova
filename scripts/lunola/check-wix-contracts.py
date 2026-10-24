"""Verify WiX installer identifier references without treating file names as IDs."""
import re
from pathlib import Path
import xml.etree.ElementTree as ET

root = Path(__file__).resolve().parents[2]
tree = ET.parse(root / 'assets/windows/concat.wxs')
identifier = re.compile(r'^[A-Za-z_][A-Za-z0-9_.]*$')
for element in tree.iter():
    if 'Id' in element.attrib:
        assert identifier.fullmatch(element.attrib['Id']), element.attrib
icons = {e.attrib['Id'] for e in tree.iter() if e.tag.endswith('}Icon')}
for element in tree.iter():
    if 'Icon' in element.attrib:
        assert element.attrib['Icon'] in icons, element.attrib
    if element.tag.endswith('}Property') and element.attrib.get('Id') == 'ARPPRODUCTICON':
        assert element.attrib['Value'] in icons, element.attrib
print('WiX identifiers and icon references PASS')
