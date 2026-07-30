# RockSonic

A CLI tool that syncs your music from a SubSonic-compatible server (Navidrome, Airsonic, etc.) to a local folder. It downloads your starred songs or a playlist, embeds cover art, and optionally converts everything to MP3.

## Requirements

- **ImageMagick 7** — for cover art processing
- **ffmpeg** — for embedding cover art into audio files

On Ubuntu/Debian:
```
sudo apt install imagemagick ffmpeg
```

## Installation

Download the latest binary from the [releases page](https://github.com/koriwi/rocksonic-rs/releases/latest), then make it executable:

```
chmod +x rocksonic-rs
```

## Usage

```
rocksonic-rs [OUTPUT_DIR] -h <host> -u <username> -p <password> [OPTIONS]
```

If no output directory is given, the current directory is used.

### Required flags

| Flag | Description |
|------|-------------|
| `-h, --host` | Server URL, **must include `/rest`** (e.g. `https://music.example.com/rest`) |
| `-u, --username` | Your username |
| `-p, --password` | Your password |

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `-l, --playlist <id>` | starred songs | Sync a specific playlist by ID instead of starred songs |
| `-m, --mp3 <kbps>` | off | Convert to MP3 at the given bitrate (e.g. `320`) |
| `-c, --coversize <px>` | `500` | Cover art size in pixels |
| `-t, --threads <n>` | `5` | Number of parallel download threads |
| `-f, --flat` | off | Put all files in one folder; artist and album are added to the filename |
| `-s, --sync` | off | Remove files from the output folder that are no longer in the playlist |
| `--config <file>` | — | Read options from a JSON config file |

### Examples

Sync starred songs to `~/Music/`:
```
rocksonic-rs ~/Music -h https://music.example.com/rest -u alice -p secret
```

Sync a playlist and convert to MP3 at 320 kbps:
```
rocksonic-rs ~/Music -h https://music.example.com/rest -u alice -p secret -l abc123 -m 320
```

Use a config file instead of passing the options each time:
```
rocksonic-rs --config ~/.config/rocksonic.json
```

The config uses the same JSON format as daemon mode. Add an `output_dir` value to choose the destination; relative paths are resolved from the config file's directory. If it is omitted, the current directory is used:

```json
{
  "output_dir": "/home/alice/Music",
  "host": "https://music.example.com/rest",
  "username": "alice",
  "password": "secret",
  "mp3": 320,
  "playlist": "abc123"
}
```

## Output structure

By default files are organized as:

```
<output_dir>/
  <playlist name>/
    <artist>/
      <album>/
        cover.jpeg
        001 Song Title.flac
        002 Another Song.flac
```

With `--flat`:
```
<output_dir>/
  <playlist name> flat/
    Artist Album 001 Song Title.flac
```

## Daemon mode

Daemon mode watches a directory for a USB drive (or any mount point) containing a `rocksonic.json` file. When a matching drive is detected, RockSonic syncs to it automatically. When the drive is removed, it waits for the next one.

```
rocksonic-rs --daemon /run/media/username
```

The `rocksonic.json` file on the drive holds the same options as the CLI flags:

```json
{
  "host": "https://music.example.com/rest",
  "username": "alice",
  "password": "secret",
  "mp3": 320,
  "coversize": 500,
  "playlist": "abc123",
  "flat": false
}
```

## Cache

Downloaded files are cached in `~/.local/share/rocksonic_songs/` so they don't need to be re-downloaded on subsequent runs. Cover art is stored in `.cover/` and raw audio in `.mp3/` (when converting) or `.cache/` (original format).
