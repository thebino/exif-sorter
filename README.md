# Exif-tool

[![License](https://img.shields.io/github/license/thebino/exif-sorter?style=for-the-badge)](./LICENSE.md)
[![GitHub contributors](https://img.shields.io/github/contributors/thebino/exif-sorter?color=success&style=for-the-badge)](https://github.com/photos-network/core/graphs/contributors)
![GitHub Workflow Status](https://img.shields.io/github/actions/workflow/status/thebino/exif-sorter/ci.yaml?style=for-the-badge)


A simple tool to read the exif data from images and sort them into sub-directories based on those data.


> ⚠️⚠️⚠️
> This application is still in Development.
>
> TODOs
> * TUI: trigger search and process
> * TUI: update progress
> * CLI: separate search and process into lib
> * CLI: re-use Image from lib instead of ImageFile in app state

## Cross compile via Docker
```shell
CROSS_CONTAINER_OPTS="--platform linux/amd64" cross build --target x86_64-unknown-linux-gnu
```
