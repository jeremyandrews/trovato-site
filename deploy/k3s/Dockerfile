# The trovato.rs site as one image, for the k3s review deployment.
#
# The pinned kernel image plus the site's overlay, copied to exactly the paths
# docker-compose.yml bind-mounts them at, so the kernel sees the same search
# paths in both places. The build context is not the repository: apply.sh
# assembles a staging directory holding plugins/, templates/, static/ and
# config/ and builds from that.
ARG KERNEL_IMAGE=ghcr.io/jeremyandrews/trovato:0.105.0
FROM ${KERNEL_IMAGE}

COPY plugins/   /site/plugins/
COPY templates/ /site/templates/
COPY static/    /site/static/
COPY config/    /site/config/

ENV PLUGINS_DIR=/app/plugins:/site/plugins \
    TEMPLATES_DIR=/app/templates:/site/templates \
    STATIC_DIR=/app/static:/site/static \
    UPLOADS_DIR=/site-uploads \
    PORT=3000
