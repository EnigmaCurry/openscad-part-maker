# openscad-part-maker

[![Coverage](https://img.shields.io/badge/Coverage-Report-purple)](https://EnigmaCurry.github.io/openscad-part-maker/coverage/master/)

This is a self-service web app for making custom 3d printed parts via
an OpenSCAD template. It presents a web form for a user to upload SVG
assets and to specify custom parameters. It has an API for processing
these inputs and downloading to the user's browser the resulting .STL
file.

## Requirements

 - The recommended installation method is with Docker or Podman:

   - Docker is recommended for servers, which runs the container in
     the root account.
     
   - Podman is recommended for workstations, which runs the container
     locally in your normal user account.

 - Alternatively, you may run the native binary for your CPU
   architecture.

## Install

You can install several ways, pick the way you like the best:

### Install for private use (without TLS)

You may install the container by itself and open up the port on the
host (e.g., port `3000`). Please note that this does not use any TLS,
so this should only be used privately:

```
## Set the Docker image to pull:
IMAGE=ghcr.io/enigmacurry/openscad-part-maker:latest

## Set the TCP port number to use:
PORT=3000

## Set the input SCAD file container path:
INPUT_SCAD=/template/tile.scad

## NB: you can use docker or podman for this:
podman run -d \
  --name openscad-part-maker \
  -p ${PORT}:${PORT} \
  ${IMAGE} \
  serve \
  --listen 0.0.0.0:${PORT} \
  --input-scad ${INPUT_SCAD}
```

Next, open your web browser to https://localhost:3000

### Install for public use with Docker (with TLS)

If you have a Docker server with Traefik Proxy installed, and you have
configured it for ACME TLS certificate generation, you can use it to
publish your service. Notice that Docker no longer needs to publish a
port because Traefik is handling the entrypoint for us, but we do need
to add the proper labels so Traefik knows how to route to this
container:

```
## Set the domain name you want Traefik to route to this container:
TRAEFIK_HOST=openscad-part-maker.example.com

## Set the Docker image to pull:
IMAGE=ghcr.io/enigmacurry/openscad-part-maker:latest

## Set the input SCAD file container path:
INPUT_SCAD=/template/tile.scad

## Basic auth tuple (plain):
USERNAME=guest
PASSWORD=hunter2

# Convert AUTH -> htpasswd bcrypt line.
AUTH="${USERNAME}:${PASSWORD}"
AUTH_HASH="$(printf "%s:%s\n" "${AUTH%%:*}" "$(openssl passwd -apr1 "${AUTH#*:}")")"

docker run -d \
  --name openscad-part-maker \
  -l traefik.enable=true \
  -l traefik.http.routers.openscad-part-maker.rule=Host\(\`${TRAEFIK_HOST}\`\) \
  -l traefik.http.routers.openscad-part-maker.entrypoints=websecure \
  -l traefik.http.routers.openscad-part-maker.tls=true \
  -l traefik.http.routers.openscad-part-maker.middlewares=openscad-part-maker-auth@docker \
  -l traefik.http.middlewares.openscad-part-maker-auth.basicauth.users="${AUTH_HASH}" \
  -l traefik.http.middlewares.openscad-part-maker-auth.basicauth.removeheader=true \
  -l traefik.http.services.openscad-part-maker.loadbalancer.server.port=3000 \
  ${IMAGE} \
  serve \
  --listen 0.0.0.0:3000 \
  --input-scad ${INPUT_SCAD}
```

Next, open your web browser to
https://openscad-part-maker.example.com:3000 (replace the domain with
the one you actually used above). Enter the Username and Password to
get in.

### Install the native binary (without Docker or Podman)

See
[Releases](https://github.com/EnigmaCurry/openscad-part-maker/releases)
and download the package you want to install.

Extract the package, and then run `./openscad-part-maker serve`.

Next, open your web browser to http://localhost:3000

## Development

Install these additional requirements for development purposes:

 - [Just](https://just.systems/man/en/packages.html)

Build the Docker image `openscad-part-maker`:

```
just build-docker
```

Start the HTTP service for development (builds/updates the image
implicitly):

```
just serve
```
