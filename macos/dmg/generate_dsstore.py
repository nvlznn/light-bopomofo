#!/usr/bin/env python3
"""Write the .DS_Store that lays out the 輕注音 disk-image window.

Follows the approach of Chromium's chrome/installer/mac/generate_dsstore.py:
the background is referenced by an Alias record built for where the image
will be mounted (/Volumes/<volume>/.background/background.jpg), with fixed
dates so the output is reproducible, and no bookmark. Finder ignores a background whose reference
points anywhere else, which is what happened with a reference recorded
while the image was mounted during the build.

Layout must match scripts/make-dmg-background.swift.
"""

import argparse
import datetime

from ds_store import DSStore
from mac_alias import (
    ALIAS_EJECTABLE_DISK,
    ALIAS_HFS_VOLUME_SIGNATURE,
    ALIAS_KIND_FILE,
    ALIAS_NO_CNID,
    Alias,
    TargetInfo,
    VolumeInfo,
)

FINDER_BACKGROUND_PICTURE = 2
FIXED_DATE = datetime.datetime(2000, 5, 7, 21, 10, 0, tzinfo=datetime.timezone.utc)


def background_alias(volume_name: str, image: str) -> bytes:
    """Alias to `image` (a path inside the volume, e.g. /.background/x.jpg)."""
    folder, _, filename = image.rpartition("/")
    volume = VolumeInfo(
        name=volume_name,
        creation_date=FIXED_DATE,
        fs_type=ALIAS_HFS_VOLUME_SIGNATURE,
        disk_type=ALIAS_EJECTABLE_DISK,
        attribute_flags=0,
        fs_id=b"\x00\x00",
        posix_path=f"/Volumes/{volume_name}",
    )
    target = TargetInfo(
        kind=ALIAS_KIND_FILE,
        filename=filename,
        folder_cnid=ALIAS_NO_CNID,
        cnid=ALIAS_NO_CNID,
        creation_date=FIXED_DATE,
        creator_code=b"\x00\x00\x00\x00",
        type_code=b"\x00\x00\x00\x00",
        folder_name=None,
        posix_path=image,
        carbon_path=f"{volume_name}:{folder.strip('/').replace('/', ':')}:{filename}",
    )
    return Alias(volume=volume, target=target).to_bytes()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("output")
    parser.add_argument("--volume-name", required=True)
    parser.add_argument("--app-name", required=True, help="e.g. 輕注音.app")
    parser.add_argument("--link-name", required=True, help="name of the /Applications link")
    parser.add_argument("--background", default="/.background/background.jpg")
    args = parser.parse_args()

    width, height = 660, 420
    bwsp = {
        "WindowBounds": f"{{{{200, 140}}, {{{width}, {height}}}}}",
        "ShowStatusBar": False,
        "ShowTabView": False,
        "ShowToolbar": False,
        "ShowPathbar": False,
        "ShowSidebar": False,
        "ContainerShowSidebar": False,
        "PreviewPaneVisibility": False,
        "SidebarWidth": 180,
    }
    icvp = {
        "viewOptionsVersion": 1,
        "backgroundType": FINDER_BACKGROUND_PICTURE,
        "backgroundImageAlias": background_alias(args.volume_name, args.background),
        "backgroundColorRed": 1.0,
        "backgroundColorGreen": 1.0,
        "backgroundColorBlue": 1.0,
        "gridOffsetX": 0.0,
        "gridOffsetY": 0.0,
        "gridSpacing": 100.0,
        "arrangeBy": "none",
        "showIconPreview": True,
        "showItemInfo": False,
        "labelOnBottom": True,
        "textSize": 13.0,
        "iconSize": 128.0,
    }

    with DSStore.open(args.output, "w+") as d:
        d["."]["vSrn"] = ("long", 1)
        d["."]["bwsp"] = bwsp
        d["."]["icvp"] = icvp
        d["."]["icvl"] = (b"type", b"icnv")  # icon view
        d[args.app_name]["Iloc"] = (170, 200)
        d[args.link_name]["Iloc"] = (490, 200)


if __name__ == "__main__":
    main()
