# oggify_csv
Update 7 November, 2023:  I broke it yesterday, oops.  Also, fix file length issues due to like 50 artists on one track.

Download Spotify CSV playlists to M3U + Ogg Vorbis (with a premium account).

This is a fork of what used to be [oggify](https://github.com/pisto/oggify).

This uses an older version of [librespot](https://github.com/librespot-org/librespot).

This code is some hacky bullshit, plz don't judge me.

# Usage
First, export your playlists with [Exportify](https://watsonbox.github.io/exportify/) and unzip them somewhere.

oggify_csv will process each CSV file,
create a folder, save tracks inside that folder,
and generate an M3U playlist alongside the CSV file.

Tracks are named like this: `{artist}-{album}(year)-{disc}-{track}-{track name}.ogg`

For instance: `Bil_Bless-Life_Mechanism_(1_of_2)(2009)-D01-T02-Wanting_You.ogg`

update 2023-09-14:

if file name length is > 140, files are named {artist} - {track name}.ogg

adjust as needed.

```
oggify_csv spotify-premium-user spotify-premium-password path_to_CSVs (Optional)
```
Existing files are skipped and not redownloaded.
If a path to the CSV files isn't provided, it will search wherever the binary is at.
Subfolders are searched too.
You soul is also searched, but DirectoryNotFound.

# Changes from Oggify
* core/src/spotify_id.rs changed to use std u128, was using some old ass crate to do it before.

* librespot a3c63b4e055f3ec68432d4a27479bed102e68e9e files are now local. because.

* The CSV/M3U shit, obviously.

* File names are mostly sanitized for Windows.

* The code is formatted and 10x uglier than before.

## Should this exist?
Probably not.  Go support the artists.  I made this for selfish reasons.

I'm sharing this is so that people don't pay for malware to do the same thing.

## Will you update or otherwise support this?
Fuck no, I spent as much of my life on this shit thing as I ever intend to.
If it's missing functionality or buggy, fix it.
Pull requests to make the code uglier might be accepted.