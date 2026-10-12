# syntax=docker/dockerfile:1
# Both stages use the requested target platform, including under BuildKit emulation.
FROM rust:1-bookworm AS build
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked --bin my-ntr-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates curl libgcc-s1 \
  && rm -rf /var/lib/apt/lists/* \
  && groupadd --gid 1000 ntr \
  && useradd --uid 1000 --gid 1000 --no-create-home --shell /usr/sbin/nologin ntr \
  && mkdir /data \
  && chown 1000:1000 /data
COPY --from=build /build/target/release/my-ntr-server /usr/local/bin/my-ntr-server
ARG VERSION
ARG REVISION
ARG SOURCE=https://git.lan/ntr/ntr-server
LABEL org.opencontainers.image.version=$VERSION \
  org.opencontainers.image.revision=$REVISION \
  org.opencontainers.image.source=$SOURCE
ENV HOST=0.0.0.0 PORT=8080 \
  TODO_FILE=/data/todo.txt NOTE_FILE=/data/Note.md LABELS_FILE=/data/labels.json
WORKDIR /data
USER 1000:1000
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/my-ntr-server"]
