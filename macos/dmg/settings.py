# dmgbuild settings for the 輕注音 disk image: the app on the left, an
# Applications link on the right, over a background drawn by
# scripts/make-dmg-background.swift (same coordinates).
# Called from macos/package-dmg.sh with -D app=… -D background=… -D icon=…

app = defines["app"]  # noqa: F821 (provided by dmgbuild)
app_name = app.rstrip("/").split("/")[-1]

format = "UDZO"
filesystem = "HFS+"
files = [app]
hide_extensions = [app_name]
symlinks = {"Applications": "/Applications"}
icon = defines["icon"]  # noqa: F821

background = defines["background"]  # noqa: F821
window_rect = ((200, 140), (660, 420))
default_view = "icon-view"
show_status_bar = False
show_tab_view = False
show_toolbar = False
show_pathbar = False
show_sidebar = False
icon_size = 128
text_size = 13
icon_locations = {
    app_name: (170, 200),
    "Applications": (490, 200),
}
