# openscad-part-maker

This is a self-service web frontend for making custom 3d printed parts
via an OpenSCAD template. It presents a web form for a user to upload
SVG assets and to specify custom parameters. It has an API for
processing these inputs and downloading to the user's browser the
resulting .STL file.

## Requirements

 - You will need Docker or Podman. 

   - Docker is recommended for servers, which runs the container in
     the root account.
     
   - Podman is recommended for workstations, which runs the container
     locally in your normal user account.

## Install

### Install for private use (without TLS)

You may install the container by itself and open up the port on the
host (e.g., port `3000`). Please note that this does not use any TLS,
so this should only be used privately:

```
## NB: you can use docker or podman for this.
podman run -d \
  --name openscad-part-maker \
  -p 3000:3000 \
  ${IMAGE:-ghcr.io/enigmacurry/openscad-part-maker:latest} \
  serve \
  --listen 0.0.0.0:3000 \
  --input-scad ${INPUT_SCAD:-/template/tile.scad}
```

### Install for public use with Docker (with TLS)

If you have a Docker server with Traefik Proxy installed, and you have
configured it for ACME TLS certificate generation, you can use it to
publish your service. Notice that Docker no longer needs to publish a
port because Traefik is handling the entrypoint for us, but we do need
to add the proper labels so Traefik knows how to route to this
container:

```
## Set the domain name you want Traefik to route to this container
TRAEFIK_HOST=openscad-part-maker.example.com

docker run -d \
  --name openscad-part-maker \
  -l traefik.enable=true \
  -l traefik.http.routers.openscad-part-maker.rule=Host\(\`${TRAEFIK_HOST}\`\) \
  -l traefik.http.routers.openscad-part-maker.entrypoints=websecure \
  -l traefik.http.routers.openscad-part-maker.tls=true \
  -l traefik.http.services.openscad-part-maker.loadbalancer.server.port=3000 \
  ${IMAGE:-ghcr.io/enigmacurry/openscad-part-maker:latest} \
  serve \
  --listen 0.0.0.0:3000 \
  --input-scad ${INPUT_SCAD:-/template/tile.scad}
```


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
