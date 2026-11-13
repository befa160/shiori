# Image de Shiori (lot 6.3, Q36) : le binaire musl statique de la release, sans shell ni
# paquets, en utilisateur non root (65532). Construite par `release.yml` à partir d'un contexte
# qui contient `amd64/shiori`, `arm64/shiori` et un dossier `empty/`.
FROM gcr.io/distroless/static-debian13:nonroot

ARG TARGETARCH
COPY --chmod=755 ${TARGETARCH}/shiori /usr/local/bin/shiori
# Dossiers inscriptibles par tout utilisateur (comme /tmp) : 65532 par défaut, ou celui de
# `--user` / `user:` quand un dossier n'est pas monté.
COPY --chown=65532:65532 --chmod=1777 empty /data
COPY --chown=65532:65532 --chmod=1777 empty /cache

# `SHIORI_ALLOWED_HOSTS` garde la parade au DNS rebinding active malgré l'écoute réseau :
# ajouter les noms par lesquels on ouvre Shiori (`docker run -e`, ou compose.yaml).
ENV SHIORI_DATA_DIR=/data \
    SHIORI_CACHE_DIR=/cache \
    SHIORI_ALLOWED_HOSTS=localhost
VOLUME ["/data", "/cache"]
EXPOSE 7878
ENTRYPOINT ["/usr/local/bin/shiori"]
CMD ["serve", "--listen", "0.0.0.0:7878"]
