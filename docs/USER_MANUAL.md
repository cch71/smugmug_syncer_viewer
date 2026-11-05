# SmugMug Archive Viewer: user manual

SmugMug Archive Viewer lets you browse a backup of your SmugMug account made by
[`smugmug_syncer`](https://github.com/cch71/smugmug_syncer). You can view your folders,
albums, photos and videos, and search them, on your own computer and without an internet
connection.

The viewer only reads the archive and never changes it.

- [Installing](#installing)
- [Opening an archive](#opening-an-archive)
- [Browsing](#browsing)
- [Viewing photos and videos](#viewing-photos-and-videos)
- [The info panel](#the-info-panel)
- [Searching](#searching)
- [Where the viewer keeps its own files](#where-the-viewer-keeps-its-own-files)
- [Troubleshooting](#troubleshooting)
- [Keyboard shortcuts](#keyboard-shortcuts)

## Installing

Download the package for your system from the
[Releases page](https://github.com/cch71/smugmug_syncer_viewer/releases). `SHA256SUMS.txt`
lists a checksum for each file if you want to verify your download.

| System | Download | Install |
| --- | --- | --- |
| Windows 10/11 | `SmugmugSyncerViewer_<version>_x64.msi` or `…_x64-setup.exe` | Run it |
| macOS, Apple silicon (M1 and later) | `SmugmugSyncerViewer_<version>_aarch64.dmg` | Open the DMG and drag the app to Applications |
| macOS, Intel | `SmugmugSyncerViewer_<version>_x86_64.dmg` | Same as above |
| Ubuntu 24.04+, Debian 13+ | `smugmug-syncer-viewer_<version>_amd64.deb` | `sudo apt install ./smugmug-syncer-viewer_*_amd64.deb` |
| Other Linux | `smugmug_syncer_viewer_<version>_x86_64.AppImage` (when provided) | `chmod +x` the file, then run it |

Both Linux packages need glibc 2.39 or newer: Ubuntu 24.04, Linux Mint 22, Debian 13,
Fedora 40 or later. They won't start on older systems such as Ubuntu 22.04 or Debian 12. Run
`ldd --version` to see your glibc version.

The `….app.zip` files hold the same macOS app as the DMG, for anyone who prefers a zip.

The packages aren't code-signed, so your system will warn you the first time you open the
app:

- **Windows:** SmartScreen shows "Windows protected your PC". Click **More info**, then
  **Run anyway**.
- **macOS:** the system says the app "can't be opened" or "is from an unidentified developer".
  Open **System Settings → Privacy & Security**, scroll down and click **Open Anyway**.
  You can also run `xattr -dr com.apple.quarantine "/Applications/SmugmugSyncerViewer.app"`
  in Terminal.
- **Linux:** the app needs WebKitGTK 4.1 (`libwebkit2gtk-4.1-0`). The `.deb` installs it
  for you. On other distributions, install WebKitGTK 4.1 with your package manager.

## Opening an archive

An archive is the folder you gave `smugmug_syncer` to sync into. It contains a hidden
`.smugmug_db` folder and an `artifacts` folder.

The first time you start the viewer, it asks for the archive:

1. Click **Browse…** and choose the archive folder, or type its path and click **Open**.
   A path starting with `~/` means your home folder.
2. Wait while the viewer reads the archive. This takes about a second for an archive of
   50,000 photos.

From then on, the viewer reopens the same archive each time it starts. To switch archives,
click **Open…** at the top right.

If the viewer says the folder "isn't a smugmug_syncer archive", you picked a folder above or
below the archive. Choose the folder that contains `.smugmug_db`.

You can also pass the archive when starting the viewer from a terminal:

```bash
smugmug_syncer_viewer ~/Pictures/SmugMugBackup
```

Or set the `SMUGMUG_ARCHIVE` environment variable to the archive's path.

## Browsing

The window has three parts:

- **Top bar:** the menu button (☰) shows or hides the sidebar. Click **SmugMug Archive** to
  go to the home page. The search box is in the middle, and on the right are the archive's
  location and **Open…**.
- **Sidebar:** your SmugMug folders and albums, laid out as they are on SmugMug. The number
  next to each one is how many photos and videos it holds, including everything in its
  subfolders. Click ▸ to expand a folder, or click its name to open it.
- **Main area:** the folder, album, or search results you're viewing.

The **home page** shows totals for the archive (folders, albums, photos and videos, videos)
and when it was last synced, followed by your top-level folders and albums.

A **folder** shows its subfolders and albums as cards with a cover picture. The path at the
top (Home / Family / …) takes you back up.

An **album** shows its photos and videos as a grid of thumbnails in SmugMug's order. Videos
have a ▶ badge. Large albums show 240 thumbnails at first; click **Show more** or **Show
all** at the bottom for the rest. The header shows the number of items, total size, dates
and privacy setting.

The first time you open an album, thumbnails take a moment to appear while the viewer makes
them. After that they're saved and load immediately.

## Viewing photos and videos

Click a thumbnail to open the viewer. It fills the window:

- **‹** and **›** at the sides, or the **←** and **→** keys, move to the previous or next
  item in the album.
- **✕ Close** or **Esc** returns to the album.
- **ⓘ Info**, or the **i** key, shows or hides the [info panel](#the-info-panel).
- The counter at the top (for example `9 / 55`) shows where you are in the album.

Photos are shown full size, scaled to fit the window and rotated the way the camera held
them.

Videos play with the usual controls: play/pause, the seek bar, volume and full screen.
Before you press play, the video's poster frame is shown. The archive holds SmugMug's MP4
copy of each video, so a video uploaded as `.wmv`, `.mpg` or `.3gp` still plays.

## The info panel

The info panel lists everything known about the photo or video.

| Row | Meaning |
| --- | --- |
| File | Name of the file when it was uploaded to SmugMug |
| Taken / Recorded | When the photo was taken or the video recorded |
| Uploaded | When it was uploaded to SmugMug |
| Dimensions, Size | Original width × height, and file size |
| Duration, Video, Audio | For videos: length, video codec (with profile, resolution and frame rate), and audio codec (with channels and sample rate) |
| Camera, Lens, Exposure | Camera model, lens, and aperture · shutter speed · ISO · focal length |
| Location | Where it was taken. Click it to open the spot on OpenStreetMap. |
| Format, Image key, MD5 | SmugMug's file type, its ID for the image, and the file's checksum |
| Keywords | SmugMug keywords. Click one to search for it. |
| In albums | Every album containing this image. A photo can be in several. |

### Where the information comes from

There are two sources of information about each picture:

1. **SmugMug's metadata:** the EXIF/IPTC details SmugMug read when the file was uploaded,
   which `smugmug_syncer` saves in the archive. SmugMug keeps these even when the file it
   lets you download has had its EXIF removed.
2. **The file itself:** the EXIF stored inside the downloaded photo, or, for videos, the
   codec information in the video file.

The viewer combines them. Each row above uses SmugMug's value when it has one, and the
file's when it doesn't.

Click **Show all metadata** to list every field from both sources. Each field is labelled:

- **S**: only SmugMug has this field.
- **F**: only the file has it (for example GPS speed and direction, or lens specification).
- **S+F**: both have it and they agree. Small formatting differences, such as `f/1.8` and
  `1.8`, still count as agreeing.

When the two disagree, SmugMug's value is shown first and the file's value (**F**) appears
underneath. For example, the focal length might show `4.0 mm` from SmugMug and `3.99 mm` from
the file.

## Searching

Type in the search box at the top and press **Enter**. The viewer searches file names,
titles, captions, keywords and album names.

- Searches ignore upper and lower case.
- Every word must match. `christmas tree` finds items containing both words, in any order.
- Matching folders and albums are listed first, then matching photos and videos (up to
  2,000).
- Clicking a keyword in the info panel searches for that keyword.

## Where the viewer keeps its own files

The viewer never writes into the archive. It keeps two things of its own:

| | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Settings (the last archive opened) | `~/Library/Application Support/dev.c13s.smugmug_syncer_viewer/settings.json` | `%APPDATA%\c13s\smugmug_syncer_viewer\config\settings.json` | `~/.config/smugmug_syncer_viewer/settings.json` |
| Thumbnail cache | `~/Library/Caches/dev.c13s.smugmug_syncer_viewer/thumbs/` | `%LOCALAPPDATA%\c13s\smugmug_syncer_viewer\cache\thumbs\` | `~/.cache/smugmug_syncer_viewer/thumbs/` |

The thumbnail cache uses about 40 KB per photo, about 2 GB for 50,000 photos. You can delete
it at any time to free space; thumbnails are recreated when you next view them. If a photo
in the archive changes, its thumbnail is rebuilt automatically.

## Troubleshooting

**A thumbnail or photo shows as a blank or broken image.**
The file is missing from the archive's `artifacts` folder, or it's damaged. To download
missing files, run:

```bash
smugmug-syncer --syncto <archive> sync --download-artifacts
```

To find damaged files, run:

```bash
smugmug-syncer --syncto <archive> query --check-for-invalid-artifacts
```

**A video doesn't play.**
The video file (`artifacts/<key>.mp4`) may not have been downloaded. Only the poster frame is
shown in that case. The info panel shows "Couldn't read the file" when the video file is
missing. Download it with `smugmug-syncer --syncto <archive> sync --download-artifacts`.

**Location, camera or Show all metadata are missing.**
The syncer saves SmugMug's metadata in a separate step:

```bash
smugmug-syncer --syncto <archive> sync --download-image-metadata
```

Until you run it, the viewer shows what it can read from the files themselves.

**The viewer opens the wrong archive at start-up.**
Click **Open…** and choose the right one. The viewer remembers your latest choice.

**On Linux, the window is blank or the viewer won't start.**
Make sure WebKitGTK 4.1 is installed (`libwebkit2gtk-4.1-0` on Debian/Ubuntu). On some
graphics drivers, setting `WEBKIT_DISABLE_DMABUF_RENDERER=1` before starting the viewer
helps.

## Keyboard shortcuts

| Key | Where | Action |
| --- | --- | --- |
| **←** / **→** | Photo viewer | Previous / next item in the album |
| **Esc** | Photo viewer | Back to the album |
| **i** | Photo viewer | Show or hide the info panel |
| **Enter** | Search box | Search |
