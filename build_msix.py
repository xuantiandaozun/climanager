import os, zipfile, xml.etree.ElementTree as ET
from PIL import Image
import hashlib, base64, struct

APP_NAME = "CLI Manager"
APP_VERSION = "0.1.0.0"
PUBLISHER = "CN=CLI Manager"
PUBLISHER_DISPLAY = "CLI Manager"
IDENTITY_NAME = "Dev.CLIManager"
LOGO_PATH = "logo.png"
EXE_PATH = "src-tauri/target/release/climanager.exe"
OUTPUT = f"src-tauri/target/release/bundle/msix/CLIManager_{APP_VERSION}_x64.msix"
BUILD_DIR = "build_msix_tmp"

os.makedirs(f"{BUILD_DIR}/Assets", exist_ok=True)

img = Image.open(LOGO_PATH)
sizes = {
    "StoreLogo.png": 50,
    "Square44x44Logo.png": 44,
    "Square150x150Logo.png": 150,
    "Square310x310Logo.png": 310,
    "Wide310x150Logo.png": (310, 150),
    "LockScreenLogo.png": 24,
}
for name, size in sizes.items():
    if isinstance(size, tuple):
        img.resize(size, Image.LANCZOS).save(f"{BUILD_DIR}/Assets/{name}")
    else:
        img.resize((size, size), Image.LANCZOS).save(f"{BUILD_DIR}/Assets/{name}")

manifest = ET.Element("Package", xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10")
ET.register_namespace("", "http://schemas.microsoft.com/appx/manifest/foundation/windows10")
ET.register_namespace("uap", "http://schemas.microsoft.com/appx/manifest/uap/windows10")
ET.register_namespace("rescap", "http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities")

identity = ET.SubElement(manifest, "Identity")
identity.set("Name", IDENTITY_NAME)
identity.set("Publisher", PUBLISHER)
identity.set("Version", APP_VERSION)

props = ET.SubElement(manifest, "Properties")
ET.SubElement(props, "DisplayName").text = APP_NAME
ET.SubElement(props, "PublisherDisplayName").text = PUBLISHER_DISPLAY
ET.SubElement(props, "Logo").text = "Assets/StoreLogo.png"

resources = ET.SubElement(manifest, "Resources")
ET.SubElement(resources, "Resource").set("Language", "zh-cn")
ET.SubElement(resources, "Resource").set("Language", "en-us")

apps = ET.SubElement(manifest, "Applications")
app = ET.SubElement(apps, "Application")
app.set("Id", "App")
app.set("Executable", "climanager.exe")
app.set("EntryPoint", "Windows.FullTrustApplication")
uv = ET.SubElement(app, "uap:VisualElements")
uv.set("DisplayName", APP_NAME)
uv.set("Square150x150Logo", "Assets/Square150x150Logo.png")
uv.set("Square44x44Logo", "Assets/Square44x44Logo.png")
uv.set("Description", "Local-first AI CLI workspace manager")
uv.set("BackgroundColor", "transparent")

caps = ET.SubElement(manifest, "Capabilities")
ET.SubElement(caps, "rescap:Capability").set("Name", "runFullTrust")

tree = ET.ElementTree(manifest)
ET.indent(tree, space="  ")
tree.write(f"{BUILD_DIR}/AppxManifest.xml", encoding="utf-8", xml_declaration=True)

ct = ET.Element("Types", attrib={"xmlns": "http://schemas.openxmlformats.org/package/2006/content-types"})
for ext, ct_val in [("exe", "application/vnd.microsoft.application"), ("dll", "application/vnd.microsoft.application"), ("png", "image/png"), ("xml", "application/xml")]:
    e = ET.SubElement(ct, "Default")
    e.set("Extension", ext)
    e.set("ContentType", ct_val)
e = ET.SubElement(ct, "Override")
e.set("PartName", "/AppxManifest.xml")
e.set("ContentType", "application/vnd.ms-appx.manifest+xml")
tree2 = ET.ElementTree(ct)
ET.indent(tree2, space="  ")
tree2.write(f"{BUILD_DIR}/[Content_Types].xml", encoding="utf-8", xml_declaration=True)

os.makedirs(os.path.dirname(OUTPUT), exist_ok=True)
with zipfile.ZipFile(OUTPUT, "w", zipfile.ZIP_DEFLATED) as zf:
    for root, _, files in os.walk(BUILD_DIR):
        for file in files:
            filepath = os.path.join(root, file)
            arcname = os.path.relpath(filepath, BUILD_DIR)
            zf.write(filepath, arcname)
    zf.write(EXE_PATH, "climanager.exe")

import shutil
shutil.rmtree(BUILD_DIR, ignore_errors=True)
print(f"MSIX created: {OUTPUT}")
