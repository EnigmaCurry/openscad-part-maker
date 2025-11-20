# openscad-part-maker

[![Coverage](https://img.shields.io/badge/Coverage-Report-purple)](https://EnigmaCurry.github.io/openscad-part-maker/coverage/master/)

This is a self-service web frontend for making custom 3d printed parts
via an OpenSCAD template. It presents a web form for a user to upload
SVG assets and to specify custom parameters. It has an API for
processing these inputs and downloading to the user's browser the
resulting .STL file.

## Requirements

 - Docker server and workstation to run docker commands from.
 - Just (get from
   [https://just.systems](https://just.systems/man/en/packages.html))

## Development

Build the Docker image `openscad-part-maker`:

```
just build-docker
```

Start the HTTP service for development (builds Docker image implicitly):

```
just serve
```
