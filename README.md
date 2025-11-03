# Exif-tool

[![License](https://img.shields.io/github/license/thebino/exif-sorter?style=for-the-badge)](./LICENSE.md)
[![GitHub contributors](https://img.shields.io/github/contributors/thebino/exif-sorter?color=success&style=for-the-badge)](https://github.com/thebino/exif-sorter/graphs/contributors)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/thebino/exif-sorter/ci.yaml?style=for-the-badge)


Exif-sorter is a simple tool to read the exif data from images and sort them into sub-directories based on the `DateTimeOriginal` which indicates the date/time when original image was taken.
If this date is not found, the Files modification and creation date can be used instead.

## Usage
```bash
exif-sorter -s unsorted_images -t sorted_images cli
```


> ⚠️⚠️⚠️
> This application is still in Development.
>
> TODOs
> * TUI: trigger search and process
> * TUI: update progress
> * CLI: separate search and process into lib
> * CLI: re-use Image from lib instead of ImageFile in app state

<img src="tui.png"/>

<img src="cli.png"/>

## Cross compile via Docker
Install cross
```shell
cargo install cross --git https://github.com/cross-rs/cross
```


Build w/ cross
```shell
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target x86_64-unknown-linux-musl
```

